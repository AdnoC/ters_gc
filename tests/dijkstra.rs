extern crate terse;
extern crate smallvec;
extern crate priority_queue;

use priority_queue::PriorityQueue;
use smallvec::SmallVec;
use terse::*;
use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::cmp::{PartialEq, Eq};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// NOTE: Might have problems with SmallVec not clearing values of `remove`d entries


type GcNode<'a> = PrintWrapper<Gc<'a, Node<'a>>>;
type GcEdge<'a> = Gc<'a, Edge<'a>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PrintWrapper<P>
where P: Deref, <P as Deref>::Target: fmt::Debug {
    ptr: P
}

impl<P> fmt::Debug for PrintWrapper<P> 
    where P: Deref, <P as Deref>::Target: fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", &*self.ptr)
    }
}

impl<P> Deref for PrintWrapper<P>
    where P: Deref, <P as Deref>::Target: fmt::Debug {
    type Target = <P as Deref>::Target;

    fn deref(&self) -> &Self::Target {
        &*self.ptr
    }
}

// impl<P> PartialEq for PrintWrapper<P>
// where P: Deref + PartialEq, <P as Deref>::Target: fmt::Debug {
//     fn eq(&self, other: &Self) -> bool {
//         self.ptr == other.ptr
//     }
// }
// impl<P> Eq for PrintWrapper<P>
// where P: Deref + PartialEq, <P as Deref>::Target: fmt::Debug {}

#[derive(Default)]
struct Graph<'a> {
    nodes: SmallVec<[GcNode<'a>; 16]>,
}

impl<'a> Graph<'a> {
    fn new() -> Graph<'a> {
        Default::default()
    }

    fn new_node(&mut self, proxy: &mut Proxy<'a>, name: &'static str) -> GcNode<'a> {
        assert!(self.nodes.len() < self.nodes.inline_size() - 1);
        let node = Node {
            adjacencies: RefCell::new(SmallVec::new()),
            name,
        };
        let node = PrintWrapper{ptr: proxy.store(node)};
        self.nodes.push(node.clone());
        node
    }

    // Removes all references to a node from everything in the graph.
    // Since we are using a GC its fine if references to it exist outside of us.
    fn remove_node(&mut self, name: &str) -> Option<GcNode<'a>> {
        let idx = self.nodes.iter().position(|node| node.name == name);
        idx.map(|idx| self.nodes.remove(idx))
    }

    fn node_by_name(&self, name: &str) -> Option<GcNode<'a>> {
        self.nodes.iter().find(|node| node.name == name).cloned()
    }


    fn path_for(&self, src: GcNode<'a>, dest: GcNode<'a>) -> Option<SmallVec<[GcNode<'a>; 16]>> {
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
        let mut distances: HashMap<GcNode<'a>, u64> = self.nodes.iter()
            .cloned()
            .map(|node| (node, std::u64::MAX))
            .collect();
        *distances.get_mut(&src).unwrap() = 0;
        let mut prev_in_path: HashMap<GcNode<'a>, GcNode<'a>> = HashMap::new();
        let mut nodes_to_process: PriorityQueue<GcNode<'a>, u64> = self.nodes.iter()
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
                    nodes_to_process.change_priority(&edge.dest, new_next_dist);
                }
            }
        }

        // Building the path
        if !prev_in_path.contains_key(&dest) {
            return None;
        }

        let mut path = SmallVec::new();
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
    adjacencies: RefCell<SmallVec<[GcEdge<'a>; 16]>>,
    name: &'static str,
}

impl<'a> Node<'a> {
    fn connect_to(&self, proxy: &mut Proxy<'a>, dest: GcNode<'a>, weight: u32) {
        assert!(self.adjacencies.borrow().len() < self.adjacencies.borrow().inline_size() - 1);
        let edge = proxy.store(Edge { dest, weight });
        self.adjacencies.borrow_mut().push(edge);
    }

    fn weight_to(&self, dest: GcNode<'a>) -> Option<u32> {
        self.adjacencies.borrow().iter()
            .find(|edge| edge.dest == dest)
            .map(|edge| edge.weight)
    }
}

fn connect_bidirectional<'a>(proxy: &mut Proxy<'a>, a: GcNode<'a>, b: GcNode<'a>, weight: u32) {
    a.connect_to(proxy, b.clone(), weight);
    b.connect_to(proxy, a, weight);
}

impl<'a> fmt::Debug for Node<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Node {{ name: {}, adjacencies: {} }}", self.name, self.adjacencies.borrow().len())
    }
}

impl<'a> PartialEq for Node<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && &*self.adjacencies.borrow() == &*other.adjacencies.borrow()
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
const ORD: &str = "Chicago";
const PHL: &str = "Philadelphia";
const DCA: &str = "Washington, D.C.";
const SAN: &str = "San Diego";

// PATH: 1 -> 3 -> 6 -> 5   COST: 20
//          1   2   3   4   5   6   7   8   9   10  11  12  13  14  15  16
//         DTW ATL IAH JFK SFO LAS MCO PHX MIA DEN LAX BOS ORD PHL DCA SAN
// 1   DTW      7   9           14
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


#[test]
fn dijkstra_is_cool() {
    let body = |mut proxy: Proxy| {
        let mut graph = Graph::new();

        initialize_graph(&mut proxy, &mut graph);
        test_first_path(&graph);
    };

    let mut col = Collector::new();
    unsafe { col.run_with_gc(body); }

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
        // println!("v = {:?}", v);
    }

    fn test_first_path<'a>(graph: &Graph<'a>) {
        let dtw = graph.node_by_name(DTW).unwrap();
        let sfo = graph.node_by_name(SFO).unwrap();
        let path = graph.path_for(dtw.clone(), sfo.clone()).expect("was unable to find a path");

        let iah = graph.node_by_name(IAH).unwrap();
        let las = graph.node_by_name(LAS).unwrap();

        let expected = [dtw, iah, las, sfo];
        assert_eq!(&expected, &*path);

        let path_weight: u32 = path.iter()
            .zip(path.iter().skip(1).cloned())
            .map(|(src, dst)| src.weight_to(dst).unwrap())
            .sum();
        assert_eq!(20, path_weight);
    }


    unimplemented!()
}
