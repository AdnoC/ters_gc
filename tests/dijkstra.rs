extern crate smallvec;
extern crate terse;

use smallvec::SmallVec;
use terse::*;
use std::cell::RefCell;

// NOTE: Might have problems with SmallVec not clearing values of `remove`d entries


type GcNode<'a> = Gc<'a, Node<'a>>;
type GcEdge<'a> = Gc<'a, Edge<'a>>;

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
        let node = proxy.store(node);
        self.nodes.push(node.clone());
        node
    }

    // Removes all references to a node from everything in the graph.
    // Since we are using a GC its fine if references to it exist outside of us.
    fn remove_node(&mut self, name: &str) -> Option<GcNode<'a>> {
        let idx = self.nodes.iter().position(|node| node.name == name);
        idx.map(|idx| self.nodes.remove(idx))
    }

    fn node_by_name(&mut self, name: &str) -> Option<GcNode<'a>> {
        self.nodes.iter().find(|node| node.name == name).cloned()
    }
}

#[derive(Default, Clone)]
struct Node<'a> {
    adjacencies: RefCell<SmallVec<[GcEdge<'a>; 16]>>,
    name: &'static str,
}

impl<'a> Node<'a> {
    fn connect_to(&mut self, proxy: &mut Proxy<'a>, dest: GcNode<'a>, weight: u32) {
        assert!(self.adjacencies.borrow().len() < self.adjacencies.borrow().inline_size() - 1);
        let edge = proxy.store(Edge { dest, weight });
        self.adjacencies.borrow_mut().push(edge);
    }
}

#[derive(Clone)]
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


#[test]
fn dijkstra_is_cool() {
    let body = |mut proxy: Proxy| {
        let mut graph = Graph::new();
        graph.new_node(&mut proxy, DTW);
        graph.new_node(&mut proxy, ATL);
        graph.new_node(&mut proxy, ORD);
        graph.new_node(&mut proxy, JFK);
        graph.new_node(&mut proxy, SFO);
        graph.new_node(&mut proxy, LAS);
    };

    let mut col = Collector::new();
    unsafe { col.run_with_gc(body); }


    unimplemented!()
}
