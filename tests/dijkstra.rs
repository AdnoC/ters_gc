extern crate smallvec;
extern crate terse;

use smallvec::SmallVec;
use terse::*;
use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::cmp::{ PartialEq, Eq };

// NOTE: Might have problems with SmallVec not clearing values of `remove`d entries


type GcNode<'a> = PrintWrapper<Gc<'a, Node<'a>>>;
type GcEdge<'a> = Gc<'a, Edge<'a>>;

#[derive(Clone, Copy, PartialEq, Eq)]
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

    fn path_to(&self, dest: GcNode<'a>) -> Option<SmallVec<[GcNode<'a>; 16]>> {
        unimplemented!()
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


#[derive(Clone, PartialEq, Eq)]
struct Edge<'a> {
    dest: GcNode<'a>,
    weight: u32,
}

// Cities by airport code
const DTW: &str = "Detroit";
const ATL: &str = "Atlanta";
const ORD: &str = "Chicago";
const JFK: &str = "New York";
const SFO: &str = "San Francisco";
const LAS: &str = "Las Vegas";
const MCO: &str = "Orlando";
const PHX: &str = "Pheonix";
const MIA: &str = "Miami";
const DEN: &str = "Denver";
const LAX: &str = "Los Angeles";
const BOS: &str = "Boston";
const IAH: &str = "Houston";
const PHL: &str = "Philadelphia";
const DCA: &str = "Washington, D.C.";
const SAN: &str = "San Diego";

// PATH: 1 -> 3 -> 6 -> 5   COST: 20
//          1   2   3   4   5   6   7   8   9   10  11  12  13  14  15  16
//         DTW ATL ORD JFK SFO LAS MCO PHX MIA DEN LAX BOS IAH PHL DCA SAN
// 1   DTW      7   9           14
// 2   ATL  7       10  15
// 3   ORD  9   10      11      2
// 4   JFK      15  11      6
// 5   SFO              6       9
// 6   LAS  14      2       9
// 7   MCO
// 8   PHX
// 9   MIA
// 10  DEN
// 11  LAX
// 12  BOS
// 13  IAH
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
        let ord = graph.new_node(proxy, ORD);
        let jfk = graph.new_node(proxy, JFK);
        let sfo = graph.new_node(proxy, SFO);
        let las = graph.new_node(proxy, LAS);

        connect_bidirectional(proxy, dtw.clone(), atl.clone(), 7);
        connect_bidirectional(proxy, dtw.clone(), ord.clone(), 9);
        connect_bidirectional(proxy, dtw.clone(), las.clone(), 14);
        connect_bidirectional(proxy, atl.clone(), ord.clone(), 10);
        connect_bidirectional(proxy, atl.clone(), jfk.clone(), 15);
        connect_bidirectional(proxy, ord.clone(), jfk.clone(), 11);
        connect_bidirectional(proxy, ord.clone(), las.clone(), 2);
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
        let path = dtw.clone().path_to(sfo.clone()).expect("was unable to find a path");

        let ord = graph.node_by_name(ORD).unwrap();
        let las = graph.node_by_name(LAS).unwrap();

        let expected = [dtw, sfo, ord, las];
        assert_eq!(&expected, &*path);

        let path_weight: u32 = path.iter()
            .zip(path.iter().skip(1).cloned())
            .map(|(src, dst)| src.weight_to(dst).unwrap())
            .sum();
        assert_eq!(20, path_weight);
    }

    unimplemented!()
}
