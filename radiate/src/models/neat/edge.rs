
extern crate uuid;

use uuid::Uuid;


/// Edge is a connection between two nodes in the graph
/// 
/// Src is the innovation number of the node sending input through the network
/// dst is the innovation number of the node receiving the input from the src neuron
/// innov is the edge's unique innovation number for crossover and mutation
/// weight is the weight of the connection
/// active keeps track of if this edge is active or not, meaning it will be used 
/// while feeding data through the network
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub src: Uuid,
    pub dst: Uuid, 
    pub innov: Uuid,
    pub weight: f32,
    pub total_weight_delta: f32,
    pub active: bool
}


impl Edge {

    pub fn new(src: Uuid, dst: Uuid, innov: Uuid, weight: f32, active: bool) -> Self {
        Edge { 
            src,    
            dst, 
            innov, 
            weight, 
            total_weight_delta: 0.0,
            active 
        }
    }



    /// update the weight of this edge connection
    #[inline]
    pub fn update(&mut self, delta: f32, update: bool) {
        self.total_weight_delta += delta;
        if update {
            self.weight += self.total_weight_delta;
            self.total_weight_delta = 0.0;
        }
    }
}
