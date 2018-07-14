extern crate priority_queue;
extern crate ters_gc;

use priority_queue::PriorityQueue;
use std::cell::RefCell;
use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use ters_gc::trace::{Trace, Tracer};
use ters_gc::*;

type GcNode<'a> = Gc<'a, Node<'a>>;
type GcEdge<'a> = Gc<'a, Edge<'a>>;

#[derive(Default)]
struct Graph<'a> {
    nodes: Vec<GcNode<'a>>,
}

impl<'a> Graph<'a> {
    fn new() -> Graph<'a> {
        Default::default()
    }

    fn new_node(&mut self, proxy: &mut Proxy<'a>, name: &'static str) -> GcNode<'a> {
        let node = Node {
            adjacencies: RefCell::new(Vec::new()),
            name,
        };
        let node = proxy.store(node);
        self.nodes.push(node.clone());
        node
    }

    // Removes all references to a node from everything in the graph.
    // Since we are using a GC its fine if references to it exist outside of us.
    fn remove_node_by_name(&mut self, name: &str) -> Option<GcNode<'a>> {
        for node in &self.nodes {
            let idx = node
                .adjacencies
                .borrow()
                .iter()
                .position(|edge| edge.dest.name == name);
            if let Some(idx) = idx {
                node.adjacencies.borrow_mut().remove(idx);
            }
        }
        let idx = self.nodes.iter().position(|node| node.name == name);
        idx.map(|idx| self.nodes.remove(idx))
    }

    fn node_by_name(&self, name: &str) -> Option<GcNode<'a>> {
        self.nodes.iter().find(|node| node.name == name).cloned()
    }

    fn path_for(&self, src: GcNode<'a>, dest: GcNode<'a>) -> Option<Vec<GcNode<'a>>> {
        // Want lower distance -> higher priority
        fn dist_to_priority(distance: u64) -> u64 {
            std::u64::MAX - distance
        }

        // This __will__ store `Gc`s in the heap where the collector can't
        // find them. __However__ we aren't touching the collector in this
        // function (we aren't allocating new garbage collected things or
        // running it), so while in this function the gc won't collect anything.
        // So, its fine to store the nodes on the heap.
        //
        // Also, all the nodes are stored in the Graph, which is a root.
        let mut distances: HashMap<GcNode<'a>, u64> = self
            .nodes
            .iter()
            .cloned()
            .map(|node| (node, std::u64::MAX))
            .collect();
        *distances.get_mut(&src).unwrap() = 0;
        let mut prev_in_path: HashMap<GcNode<'a>, GcNode<'a>> = HashMap::new();
        let mut nodes_to_process: PriorityQueue<GcNode<'a>, u64> = self
            .nodes
            .iter()
            .cloned()
            .map(|node| {
                let dist = distances[&node];
                (node, dist_to_priority(dist))
            })
            .collect();

        while !nodes_to_process.is_empty() {
            let (cur, _) = nodes_to_process.pop().unwrap();
            let cur_dist = distances[&cur];
            for edge in cur.adjacencies.borrow().iter() {
                let cur_next_dist = distances[&edge.dest];
                let new_next_dist = cur_dist + edge.weight as u64;
                if new_next_dist < cur_next_dist {
                    *distances.get_mut(&edge.dest).unwrap() = new_next_dist;
                    *prev_in_path.entry(edge.dest.clone()).or_insert(cur.clone()) = cur.clone();
                    nodes_to_process.change_priority(&edge.dest, dist_to_priority(new_next_dist));
                }
            }
        }

        // Building the path
        if !prev_in_path.contains_key(&dest) {
            return None;
        }

        let mut path = Vec::new();
        path.push(dest);
        loop {
            if let Some(node) = prev_in_path.get(path.last().unwrap()) {
                path.push(node.clone());
            } else {
                break;
            }
        }

        path.reverse();
        Some(path)
    }
}

#[derive(Default, Clone)]
struct Node<'a> {
    adjacencies: RefCell<Vec<GcEdge<'a>>>,
    name: &'static str,
}

impl<'a> Node<'a> {
    fn connect_to(&self, proxy: &mut Proxy<'a>, dest: GcNode<'a>, weight: u32) {
        let edge = proxy.store(Edge { dest, weight });
        self.adjacencies.borrow_mut().push(edge);
    }

    fn disconnect_from(&self, dest: GcNode<'a>) {
        let idx = self
            .adjacencies
            .borrow()
            .iter()
            .position(|edge| edge.dest.name == dest.name);
        if let Some(idx) = idx {
            self.adjacencies.borrow_mut().remove(idx);
        }
    }

    fn weight_to(&self, dest: GcNode<'a>) -> Option<u32> {
        self.adjacencies
            .borrow()
            .iter()
            .find(|edge| edge.dest == dest)
            .map(|edge| edge.weight)
    }
}

impl<'a> Trace for Node<'a> {
    fn trace(&self, tracer: &mut Tracer) {
        tracer.add_target(&self.adjacencies);
    }
}

fn connect_bidirectional<'a>(proxy: &mut Proxy<'a>, a: GcNode<'a>, b: GcNode<'a>, weight: u32) {
    a.connect_to(proxy, b.clone(), weight);
    b.connect_to(proxy, a, weight);
}

impl<'a> fmt::Debug for Node<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct AdjWrapper<'b, 'a: 'b>(&'b RefCell<Vec<GcEdge<'a>>>);
        impl<'a, 'b> fmt::Debug for AdjWrapper<'a, 'b> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_list().entries(self.0.borrow().iter()).finish()
            }
        }
        let adj = AdjWrapper(&self.adjacencies);
        f.debug_struct("Node")
            .field("name", &self.name)
            .field("num_adjacencies", &self.adjacencies.borrow().len())
            .field("adjacencies", &adj)
            .finish()
    }
}

impl<'a> PartialEq for Node<'a> {
    fn eq(&self, other: &Self) -> bool {
        // Only check name since if we have a cycle we'd stack overflow otherwise
        self.name == other.name
    }
}
impl<'a> Eq for Node<'a> {}

impl<'a> Hash for Node<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Edge<'a> {
    dest: GcNode<'a>,
    weight: u32,
}
impl<'a> Trace for Edge<'a> {
    fn trace(&self, tracer: &mut Tracer) {
        tracer.add_target(&self.dest);
    }
}
impl<'a> fmt::Debug for Edge<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Edge")
            .field("dest_name", &self.dest.name)
            .field("weight", &self.weight)
            .finish()
    }
}

// Cities by airport code
const DTW: &str = "Detroit";
const ATL: &str = "Atlanta";
const IAH: &str = "Houston";
const JFK: &str = "New York";
const SFO: &str = "San Francisco";
const LAS: &str = "Las Vegas";
const MCO: &str = "Orlando";
const PHX: &str = "Pheonix";
const MIA: &str = "Miami";
const DEN: &str = "Denver";
const LAX: &str = "Los Angeles";
const BOS: &str = "Boston";
const _ORD: &str = "Chicago";
const _PHL: &str = "Philadelphia";
const _DCA: &str = "Washington, D.C.";
const _SAN: &str = "San Diego";

// PATH: 1 -> 3 -> 6 -> 5   COST: 20
//          1   2   3   4   5   6   7   8   9   10  11  12  13  14  15  16
//         DTW ATL IAH JFK SFO LAS MCO PHX MIA DEN LAX BOS ORD PHL DCA SAN
// 1   DTW     !7   9           14
// 2   ATL  7       10  15
// 3   IAH  9   10      11      2
// 4   JFK      15  11      6
// 5   SFO              6       9
// 6   LAS  14      2       9
// 7   MCO
// 8   PHX
// 9   MIA
// 10  DEN
// 11  LAX
// 12  BOS
// 13  ORD
// 14  PHL
// 15  DCA
// 16  SAN

// PATH: 1 -> 12 -> 10 -> 8 -> 4 -> 5   COST: 43
//     TO   1   2   3   4   5   6   7   8   9   10  11  12  13  14  15  16
// FROM    DTW ATL IAH JFK SFO LAS MCO PHX MIA DEN LAX BOS ORD PHL DCA SAN
// 1   DTW                      14  5                   5
// 2   ATL
// 3   IAH
// 4   JFK                  6
// 5   SFO  1           6       9
// 6   LAS  14              42              4
// 7   MCO  5                                   7
// 8   PHX              13                      16  4
// 9   MIA                      4                   4
// 10  DEN                          7   16              3
// 11  LAX                              4   4
// 12  BOS  5                                   3
// 13  ORD
// 14  PHL
// 15  DCA
// 16  SAN

#[test]
fn dijkstra_is_cool() {
    let mut col = Collector::new();
    let mut proxy = col.proxy();
    let mut graph = Graph::new();

    initialize_graph(&mut proxy, &mut graph);
    test_first_path(&graph);
    secede_texas(&mut proxy, &mut graph);
    test_second_path(&graph);

    fn initialize_graph<'a>(proxy: &mut Proxy<'a>, graph: &mut Graph<'a>) {
        let dtw = graph.new_node(proxy, DTW);
        let atl = graph.new_node(proxy, ATL);
        let iah = graph.new_node(proxy, IAH);
        let jfk = graph.new_node(proxy, JFK);
        let sfo = graph.new_node(proxy, SFO);
        let las = graph.new_node(proxy, LAS);

        connect_bidirectional(proxy, dtw.clone(), atl.clone(), 7);
        connect_bidirectional(proxy, dtw.clone(), iah.clone(), 9);
        connect_bidirectional(proxy, dtw.clone(), las.clone(), 14);
        connect_bidirectional(proxy, atl.clone(), iah.clone(), 10);
        connect_bidirectional(proxy, atl.clone(), jfk.clone(), 15);
        connect_bidirectional(proxy, iah.clone(), jfk.clone(), 11);
        connect_bidirectional(proxy, iah.clone(), las.clone(), 2);
        connect_bidirectional(proxy, jfk.clone(), sfo.clone(), 6);
        connect_bidirectional(proxy, sfo.clone(), las.clone(), 9);

        // let mut v: SmallVec<[GcNode<'a>; 16]> = SmallVec::new();
        // v.push(dtw);
        // v.push(atl);
        // v.push(ord);
        // v.push(jfk);
        // v.push(sfo);
    }

    fn test_first_path<'a>(graph: &Graph<'a>) {
        let dtw = graph.node_by_name(DTW).unwrap();
        let sfo = graph.node_by_name(SFO).unwrap();
        let path = graph
            .path_for(dtw.clone(), sfo.clone())
            .expect("was unable to find a path");

        let iah = graph.node_by_name(IAH).unwrap();
        let las = graph.node_by_name(LAS).unwrap();

        let expected = [dtw, iah, las, sfo];
        assert_eq!(&expected, &*path);

        let path_weight: u32 = path
            .iter()
            .zip(path.iter().skip(1).cloned())
            .map(|(src, dst)| src.weight_to(dst).unwrap())
            .sum();
        assert_eq!(20, path_weight);
    }

    // Texas decided to secede from the US and become its own nation,
    // a theocracy centered on the Church of BBQ. Several other states
    // followed it.
    // For some reason the US government isn't happy about this.
    // It prohibited flights to/from the seceded states.
    // To show the BBQ-ans how much cooler the US is, they decided to create new
    // airports.
    fn secede_texas<'a>(proxy: &mut Proxy<'a>, graph: &mut Graph<'a>) {
        graph.remove_node_by_name(IAH);
        graph.remove_node_by_name(ATL);
        let pre_tracked = proxy.num_tracked();
        proxy.run();
        let post_tracked = proxy.num_tracked();
        assert!(pre_tracked > post_tracked);

        let dtw = graph.node_by_name(DTW).unwrap();
        let sfo = graph.node_by_name(SFO).unwrap();
        let jfk = graph.node_by_name(JFK).unwrap();
        let las = graph.node_by_name(LAS).unwrap();

        let mco = graph.new_node(proxy, MCO);
        let phx = graph.new_node(proxy, PHX);
        let mia = graph.new_node(proxy, MIA);
        let den = graph.new_node(proxy, DEN);
        let lax = graph.new_node(proxy, LAX);
        let bos = graph.new_node(proxy, BOS);

        connect_bidirectional(proxy, dtw.clone(), mco.clone(), 5);
        connect_bidirectional(proxy, dtw.clone(), bos.clone(), 5);
        sfo.connect_to(proxy, dtw.clone(), 1);
        las.disconnect_from(sfo.clone());
        las.connect_to(proxy, sfo.clone(), 42);
        connect_bidirectional(proxy, las.clone(), mia.clone(), 4);
        connect_bidirectional(proxy, mco.clone(), den.clone(), 7);
        phx.connect_to(proxy, jfk.clone(), 13);
        connect_bidirectional(proxy, phx.clone(), den.clone(), 16);
        connect_bidirectional(proxy, phx.clone(), lax.clone(), 4);
        connect_bidirectional(proxy, mia.clone(), lax.clone(), 4);
        connect_bidirectional(proxy, den.clone(), bos.clone(), 3);
    }

    fn test_second_path<'a>(graph: &Graph<'a>) {
        let dtw = graph.node_by_name(DTW).unwrap();
        let sfo = graph.node_by_name(SFO).unwrap();
        let path = graph
            .path_for(dtw.clone(), sfo.clone())
            .expect("was unable to find a path");
        let path_weight: u32 = path
            .iter()
            .zip(path.iter().skip(1).cloned())
            .map(|(src, dst)| src.weight_to(dst).unwrap())
            .sum();
        assert_eq!(43, path_weight);
    }
}
