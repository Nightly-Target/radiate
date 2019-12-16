extern crate rand;

use std::mem;
use std::fmt;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::ptr;
use rand::Rng;
use rand::seq::SliceRandom;
use super::{
    layertype::LayerType,
    layer::Layer,
    layer::Mutate
};
use super::super::{
    neuron::Neuron,
    edge::Edge,
    neatenv::NeatEnvironment,
    counter::Counter,
    neurontype::NeuronType,
    activation::Activation
};



#[derive(Debug)]
pub struct Dense {
    pub inputs: Vec<i32>,
    pub outputs: Vec<i32>,
    pub nodes: HashMap<i32, *mut Neuron>,
    pub edges: HashMap<i32, Edge>,
    pub layer_type: LayerType,
    pub activation: Activation
}



impl Dense {

    
    /// create a new fully connected dense layer.
    /// Each input is connected to each output with a randomly generated weight attached to the connection
    #[inline]
    pub fn new(num_in: i32, num_out: i32, layer_type: LayerType, activation: Activation, counter: &mut Counter) -> Self {
        let mut layer = Dense {
            inputs: (0..num_in)
                .into_iter()
                .map(|_| counter.next())
                .collect(),
            outputs: (0..num_out)
                .into_iter()
                .map(|_| counter.next())
                .collect(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            layer_type,
            activation
        };

        for innov in layer.inputs.iter() {
            layer.nodes.insert(*innov, Neuron::new(*innov, NeuronType::Input, activation).as_mut_ptr());
        }
        for innov in layer.outputs.iter() {
            layer.nodes.insert(*innov, Neuron::new(*innov, NeuronType::Output, activation).as_mut_ptr());
        }

        let mut r = rand::thread_rng();
        for i in layer.inputs.iter() {
            for j in layer.outputs.iter() {
                let src = layer.nodes.get(i).unwrap();
                let dst = layer.nodes.get(j).unwrap();
                let weight = r.gen::<f64>() * 2.0 - 1.0;
                let new_edge = Edge::new(*i, *j, counter.next(), weight, true);
                unsafe { (**src).outgoing.push(new_edge.innov); }
                unsafe { (**dst).incoming.insert(new_edge.innov, None); }
                layer.edges.insert(new_edge.innov, new_edge);
            }
        }

        layer
    }

    

    /// reset all the neurons in the network so they can be fed forward again
    #[inline]
    unsafe fn reset_neurons(&self) {
        for val in self.nodes.values() {
            (**val).reset_neuron();
        }
    }   
    


    /// Add a node to the network by getting a random edge 
    /// and inserting the new node inbetween that edge's source
    /// and destination nodes. The old weight is pushed forward 
    /// while the new weight is randomly chosen and put between the 
    /// old source node and the new node
    #[inline]
    pub fn add_node(&mut self, counter: &mut Counter, activation: Activation) -> Option<*mut Neuron> {
        unsafe {
            // create a new node to insert inbetween the sending and receiving nodes 
            let new_node = Neuron::new(counter.next(), NeuronType::Hidden, activation).as_mut_ptr();
            // get an edge to insert the node into
            // get the sending and receiving nodes from the edge
            let curr_edge = self.edges.get_mut(&self.random_edge()).unwrap();
            let sending = self.nodes.get(&curr_edge.src).unwrap();
            let receiving = self.nodes.get(&curr_edge.dst).unwrap();
            // create two new edges that connect the src and the new node and the 
            // new node and dst, then disable the current edge 
            curr_edge.active = false;
            let incoming = Edge::new((**sending).innov, (*new_node).innov, counter.next(), 1.0, true);
            let outgoing = Edge::new((*new_node).innov, (**receiving).innov, counter.next(), curr_edge.weight, true);
            // remove the outgoing connection from the sending node
            (**sending).outgoing.retain(|x| x != &(curr_edge.innov));
            (**receiving).incoming.remove(&curr_edge.innov);
            // add the new values
            (**sending).outgoing.push(incoming.innov);
            (**receiving).incoming.insert(outgoing.innov, None);
            // add the vlaues to the new node
            (*new_node).outgoing.push(outgoing.innov);
            (*new_node).incoming.insert(incoming.innov, None);
            // add the new nodes and the new edges to the network
            self.edges.insert(incoming.innov, incoming);
            self.edges.insert(outgoing.innov, outgoing);
            self.nodes.insert((*new_node).innov, new_node);   
            Some(new_node)
        }
    }



    /// add a connection to the network. Randomly get a sending node that cannot 
    /// be an output and a receiving node which is not an input node, the validate
    /// that the desired connection can be made. If it can be, make the connection
    /// with a weight of .5 in order to minimally impact the network 
    #[inline]
    pub fn add_edge(&mut self, counter: &mut Counter) -> Option<Edge> {
        unsafe {
            // get a valid sending neuron
            let sending = loop {
                let temp = self.nodes.get(&self.random_node()).unwrap();
                if (**temp).neuron_type != NeuronType::Output {
                    break temp;
                }
            };
            // get a vaild receiving neuron
            let receiving = loop {
                let temp = self.nodes.get(&self.random_node()).unwrap();
                if (**temp).neuron_type != NeuronType::Input {
                    break temp;
                }
            };
            // determine if the connection to be made is valid 
            if self.valid_connection(sending, receiving) {
                // if the connection is valid, make it and wire the nodes to each
                let mut r = rand::thread_rng();
                let new_edge = Edge::new((**sending).innov, (**receiving).innov, counter.next(), r.gen::<f64>(), true);
                (**sending).outgoing.push(new_edge.innov);
                (**receiving).incoming.insert(new_edge.innov, None);
                // add the new edge to the network
                let result = new_edge.clone();
                self.edges.insert(new_edge.innov, new_edge);               
                return Some(result)
            }
            None
        }
    }



    /// Test whether the desired connection is valid or not by testing to see if 
    /// 1.) it is recursive
    /// 2.) the connection already exists
    /// 3.) the desired connection would create a cycle in the graph
    /// if these are all false, then the connection can be made
    #[inline]
    unsafe fn valid_connection(&self, sending: &*mut Neuron, receiving: &*mut Neuron) -> bool {
        if sending == receiving {
            return false
        } else if self.exists(sending, receiving) {
            return false
        } else if self.cyclical(sending, receiving) {
            return false
        }
        true
    }



    /// check to see if the connection to be made would create a cycle in the graph
    /// and therefore make it network invalid and unable to feed forward
    #[inline]
    unsafe fn cyclical(&self, sending: &*mut Neuron, receiving: &*mut Neuron) -> bool {
        // dfs stack which gets the receiving Neuron<dyn neurons> outgoing connections
        let mut stack = (**receiving).outgoing
            .iter()
            .map(|x| self.edges.get(x).unwrap().dst)
            .collect::<Vec<_>>();
        // while the stack still has nodes, continue
        while stack.len() > 0 {
            let curr = self.nodes.get(&stack.pop().unwrap()).unwrap();
            // if the current node is the same as the sending, this would cause a cycle
            if curr == sending {
                return true;
            }
            // else add all the current node's outputs to the stack to search through
            for i in (**curr).outgoing.iter() {
                stack.push(self.edges.get(i).unwrap().dst);
            }
        }
        false
    }



    /// check if the desired connection already exists within he network, if it does then
    /// we should not be creating the connection.
    #[inline]
    unsafe fn exists(&self, sending: &*mut Neuron, receiving: &*mut Neuron) -> bool {
        for val in self.edges.values() {
            if val.src == (**sending).innov && val.dst == (**receiving).innov {
                return true
            }
        }
        false
    }



    /// get a random node from the network - the hashmap does not have a idomatic
    /// way to do this so this is a workaround. Returns the innovation number of the node
    /// in order to satisfy rust borrow rules
    #[inline]
    fn random_node(&self) -> i32 {
        let index = rand::thread_rng().gen_range(0, self.nodes.len());
        for (i, (innov, _)) in self.nodes.iter().enumerate() {
            if i == index {
                return *innov;
            }
        }
        panic!("Failed to get random node");
    }



    /// get a random connection from the network - hashmap does not have an idomatic
    /// way to do this so this is a workaround. Returns the innovatio number of the 
    /// edge in order to satisfy rust borrowing rules
    #[inline]
    fn random_edge(&self) -> i32 {
        let index = rand::thread_rng().gen_range(0, self.edges.len());
        for (i, (innov, _)) in self.edges.iter().enumerate() {
            if i == index {
                return *innov;
            }
        }
        panic!("Failed to get random edge");
    }



    /// give input data to the input nodes in the network and return a vec
    /// that holds the innovation numbers of the input nodes for a dfs traversal 
    /// to feed forward those inputs through the network
    #[inline]
    unsafe fn give_inputs(&self, data: &Vec<f64>) -> Vec<i32> {
        assert!(data.len() == self.inputs.len());
        self.inputs.iter().zip(data.iter())
            .map(|(node_innov, input)| {
                let node = self.nodes.get(node_innov).unwrap();
                (**node).value = Some(*input);
                (**node).innov
            })
            .collect()
    }



    /// Edit the weights in the network randomly by either uniformly perturbing
    /// them, or giving them an entire new weight all together
    #[inline]
    fn edit_weights(&mut self, editable: f32, size: f64) {
        let mut r = rand::thread_rng();
        for (_, edge) in self.edges.iter_mut() {
            if r.gen::<f32>() < editable {
                edge.weight = r.gen::<f64>();
            } else {
                edge.weight *= r.gen_range(-size, size);
            }
        }
    }



    pub fn see(&self) {
        unsafe { 
            for node in self.nodes.values() {
                println!("{:?}", **node);
            }
            for edge in self.edges.values() {
                println!("{:?}", edge);
            }
        }
    }



    // if the new node has already been created in the same spot meaning an edge was deactivated
    // and replaced by two new edges connecting to a new node, but the evolutionary process 
    // has already created the same topological structure, then that node and those edges should
    // be represented by the innovation numbers already created, not new ones. This is crutial 
    // for preventing wrongful population explosion due to incorrect historical markings
    #[inline]
    unsafe fn neuron_control(child: &mut Dense, new_node: &*mut Neuron, env: &mut NeatEnvironment) -> Result<(), &'static str> {
        // check to see if this node has been created in the enviromnent before
        let new_node_incoming_edge: i32 = *(**new_node).incoming.keys().next().unwrap();
        let new_node_outgoing_edge: i32 = *(**new_node).outgoing.last().unwrap();

        // get the sending and receiving nodes because the new edges had new innovation 
        // numbers so the only way to check is by looking at the nodes themselves 
        // get the incoming and outoing edges of the new node and the stored node to replace the 
        // incoming edges dst and the outgoing edges src with the stored node's innovation
        let new_node_incoming_neuron = child.edges.get_mut(&new_node_incoming_edge).unwrap().src;
        let new_node_outgoing_neuron = child.edges.get_mut(&new_node_outgoing_edge).unwrap().dst;
        let neuron_key = (new_node_incoming_neuron, new_node_outgoing_neuron);
        // if the key is in the enviromnent, we know this mutation has been created before 
        if !env.global_nodes.contains_key(&neuron_key) {
            // add the new edges and nodes to the env
            let incoming_edge_key = (new_node_incoming_neuron, (**new_node).innov);
            let outgoing_edge_key = ((**new_node).innov, new_node_outgoing_neuron);
            env.global_edges.insert(incoming_edge_key, child.edges.get(&new_node_incoming_edge).unwrap().clone());
            env.global_edges.insert(outgoing_edge_key, child.edges.get(&new_node_outgoing_edge).unwrap().clone());
            env.global_nodes.insert(neuron_key, (**new_node).clone());
        } else if env.global_nodes.contains_key(&neuron_key) {
            let stored_node = env.global_nodes.get(&neuron_key).unwrap().clone().as_mut_ptr();
            if !child.nodes.contains_key(&(*stored_node).innov) {
                // actually get the edges from the env
                let stored_incoming_edge_key = (new_node_incoming_neuron, (*stored_node).innov);
                let stored_outgoing_edge_key = ((*stored_node).innov, new_node_outgoing_neuron);
                let stored_incoming_edge = env.global_edges.get(&stored_incoming_edge_key).unwrap().clone();
                let stored_outgoing_edge = env.global_edges.get(&stored_outgoing_edge_key).unwrap().clone();

                // get the actual incoming and outgoing neurons to replace the new node with the stored node 
                let incoming_node = child.nodes.get(&new_node_incoming_neuron).unwrap();
                let outgoing_node = child.nodes.get(&new_node_outgoing_neuron).unwrap();

                // remove the pointers to the edges that are going to be removed 
                (**incoming_node).outgoing.remove((**incoming_node).outgoing.iter().position(|&x| x == new_node_incoming_edge).unwrap());
                (**incoming_node).outgoing.push(stored_incoming_edge.innov);

                // replace the old edges with the stored edges from the global env
                (**outgoing_node).incoming.remove(&new_node_outgoing_edge);
                (**outgoing_node).incoming.insert(stored_outgoing_edge.innov, None);

                // remove the new node and it's new edges from the child and replace it with the stored ones
                child.nodes.remove(&(**new_node).innov);
                child.edges.remove(&new_node_incoming_edge);
                child.edges.remove(&new_node_outgoing_edge);

                // insert the new node and it's new edges into the network inplace of the previous new node
                child.nodes.insert((*stored_node).innov, stored_node);
                child.edges.insert(stored_incoming_edge.innov, stored_incoming_edge);
                child.edges.insert(stored_outgoing_edge.innov, stored_outgoing_edge); 
                      
                // roll back the counter three because there are three innovation numbers that we didn't use 
                env.subtract_count(3);    
            }
        } 
        Ok(())
    }



    /// Similar to neuron_control, this controls the edges of the graph in order to prevent 
    /// unwanted population explosion due to incorrect historical markings of innovation numbers
    /// that already exist within the population. This tends to tighten the number of species.
    #[inline]
    unsafe fn edge_control(child: &mut Dense, new_edge: Option<Edge>, env: &mut NeatEnvironment) {
        // if edge is None then we don't need to do anything
        if let Some(new_edge) = new_edge {
            // make a key for the env global edges
            let key = (new_edge.src, new_edge.dst);
            // if the key is in the enviromnet already then it has already been created 
            // and we need to replace the new edge in the child with the stored edge 
            if env.global_edges.contains_key(&key) {
                let stored_edge = env.global_edges.get(&key).unwrap();
                let src_neuron = child.nodes.get(&stored_edge.src).unwrap();
                let dst_neuron = child.nodes.get(&stored_edge.dst).unwrap();
                // replace the src and dst node's pointers to the new edge with the stored edge
                (**src_neuron).outgoing.remove((**src_neuron).outgoing.iter().position(|&x| x == new_edge.innov).unwrap());
                (**dst_neuron).incoming.remove(&new_edge.innov);
                // insert the stored edge into the src and dst neurons
                (**src_neuron).outgoing.push(stored_edge.innov);
                (**dst_neuron).incoming.insert(stored_edge.innov, None);
                // remove the new edge and add the stored edge into the child
                child.edges.remove(&new_edge.innov);
                child.edges.insert(stored_edge.innov, stored_edge.clone());
            } else {
                env.global_edges.insert(key, new_edge.clone());
            }
        }
    }


}




impl Layer for Dense {
    /// Feed a vec of inputs through the network, will panic! if 
    /// the shapes of the values do not match or if something goes 
    /// wrong within the feed forward process.
    #[inline]
    fn propagate(&mut self, data: &Vec<f64>) -> Option<Vec<f64>> {
        unsafe {
            // reset the network by clearing the previous outputs from the neurons 
            // this could be done more efficently if i didn't want to implement backprop
            // or recurent nodes, however this must be done this way in order to allow for the 
            // needed values for those algorithms to remain while they are needed 
            // give the input data to the input neurons and return back 
            // a stack to do a graph traversal to feed the inputs through the network
            self.reset_neurons();
            let mut path = self.give_inputs(data);
            // while the path is still full, continue feeding forward 
            // the data in the network, this is basically a dfs traversal
            while path.len() > 0 {
                // remove the top elemet to propagate it's value
                let curr_node = self.nodes.get(&path.pop()?)?;
                // no node should be in the path if it's value has not been set 
                // iterate through the current nodes outgoing connections 
                // to get its value and give that value to it's connected node
                if let Some(val) = (**curr_node).value {
                    for edge_innov in (**curr_node).outgoing.iter() {
                        // if the currnet edge is active in the network, we can propagate through it
                        let curr_edge = self.edges.get(edge_innov)?;
                        if curr_edge.active {
                            let receiving_node = self.nodes.get(&curr_edge.dst)?;
                            (**receiving_node).incoming.insert(curr_edge.innov, Some(val * curr_edge.weight));
                            // if the node can be activated, activate it and store it's value
                            // only activated nodes can be added to the path, so if it's activated
                            // add it to the path so the values can be propagated through the network
                            if (**receiving_node).is_ready() {
                                (**receiving_node).activate();
                                path.push((**receiving_node).innov);
                            }
                        }
                    }
                }
            }
            // once we've made it through the network, the outputs should all
            // have calculated their values. Gather the values and return the vec
            let mut network_output = Vec::with_capacity(self.outputs.len());
            for innov in self.outputs.iter() {
                let node_val = (**self.nodes.get(innov)?).value?;
                network_output.push(node_val);
            }
            Some(network_output)
        }
    }


    /// Backpropagation algorithm, transfer the error through the network and change the weights of the
    /// edges accordinly, this is pretty straight forward due to the design of the neat graph
    #[inline]
    fn backprop(&mut self, error: &Vec<f64>, learning_rate: f64) -> Option<Vec<f64>> {
        // feed forward the input data to get the output in order to compute the error of the network
        // create a dfs stack to step backwards through the network and compute the error of each neuron
        // then insert that error in a hashmap to keep track of innov of the neuron and it's error 
        // 
        unsafe  {
            let mut path = Vec::new();
            for (i, innov) in self.outputs.iter().enumerate() {
                let node = self.nodes.get(innov)?;
                (**node).error = Some(error[i]);
                path.push(*innov);
            }

            // step through the network backwards and adjust the weights
            while path.len() > 0 {
                // get the current node and it's error 
                let curr_node = self.nodes.get(&path.pop()?)?;
                let curr_node_error = (**curr_node).error? * learning_rate;
                // iterate through each of the incoming edes to this neuron and adjust it's weight
                // and add it's error to the errros map
                for incoming_edge_innov in (**curr_node).incoming.keys() {
                    let curr_edge = self.edges.get_mut(incoming_edge_innov)?;
                    // if the current edge is active, then it is contributing to the error and we need to adjust it
                    if curr_edge.active {
                        let src_neuron = self.nodes.get(&curr_edge.src)?;
                        let step = curr_node_error * (**curr_node).deactivate();
                        // add the weight step (gradient) * the currnet value to the weight to adjust the weight by the error
                        curr_edge.weight += step * (**src_neuron).value?;
                        (**src_neuron).error = Some(curr_edge.weight * curr_node_error);
                        path.push(curr_edge.src);
                    }
                }
            }
            let mut output = Vec::with_capacity(self.inputs.len());
            for innov in self.inputs.iter() {
                output.push((**self.nodes.get(innov)?).error?);
            }
            Some(output)
        }
    }


    
    fn as_ref_any(&self) -> &dyn Any
        where Self: Sized + 'static
    {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn Any
        where Self: Sized + 'static
    {
        self
    }

    fn shape(&self) -> (usize, usize) {
        (self.inputs.len(), self.outputs.len())
    }

    /// find the max edge innovation number from the network for determing 
    /// the genetic_structure of this network
    #[inline]
    fn max_marker(&self) -> i32 {
        let mut result = None;
        for key in self.edges.keys() {
            if result.is_none() || key > result.unwrap() {
                result = Some(key);
            }
        }
        *result.unwrap_or_else(|| panic!("Failed to find max marker"))
    }

}


impl Mutate<Dense> for Dense
    where Dense: Layer
{
    fn mutate(child: &mut  Dense, _: &Dense, parent_two: &Dense, env: &Arc<RwLock<NeatEnvironment>>, crossover_rate: f32) 
        where Self: Sized + Send + Sync
    {
        unsafe {
            let mut set = (*env).write().unwrap();
            let mut r = rand::thread_rng();
            if r.gen::<f32>() < crossover_rate {
                for (innov, edge) in child.edges.iter_mut() {
                    // if the edge is in both networks, then radnomly assign the weight to the edge
                    if parent_two.edges.contains_key(innov) {
                        if r.gen::<f32>() < 0.5 {
                            edge.weight = parent_two.edges.get(innov).unwrap().weight;
                        }
                        // if the edge is deactivated in either network and a random number is less than the 
                        // reactivate parameter, then reactiveate the edge and insert it back into the network
                        if (!edge.active || !parent_two.edges.get(innov).unwrap().active) && r.gen::<f32>() < set.reactivate.unwrap() {
                            (**child.nodes.get(&edge.src).unwrap()).outgoing.push(*innov);
                            (**child.nodes.get(&edge.dst).unwrap()).incoming.insert(*innov, None);
                            edge.active = true;
                        }
                    }
                }
            } else {
                // if a random number is less than the edit_weights parameter, then edit the weights of the network edges
                // add a possible new node to the network randomly 
                // attempt to add a new edge to the network, there is a chance this operation will add no edge
                if r.gen::<f32>() < set.weight_mutate_rate.unwrap() {
                    child.edit_weights(set.edit_weights.unwrap(), set.weight_perturb.unwrap());
                }
                // if the layer is a dense pool then it can add nodes and connections to the layer as well
                if child.layer_type == LayerType::DensePool {
                    if r.gen::<f32>() < set.new_node_rate.unwrap() {
                        let act_func = *set.activation_functions.choose(&mut r).unwrap();
                        let new_node = child.add_node(set.get_mut_counter(), act_func).unwrap();
                        Dense::neuron_control(child, &new_node, &mut set).ok().unwrap();
                    }
                    if r.gen::<f32>() < set.new_edge_rate.unwrap() {
                        let new_edge = child.add_edge(set.get_mut_counter());
                        Dense::edge_control(child, new_edge, &mut set);
                    }
                }
            }
        }
    }



    fn distance(one: &Dense, two: &Dense, env: &Arc<RwLock<NeatEnvironment>>) -> f64 {
        // keep track of the number of excess and disjoint genes and the
        // average weight of shared genes between the two networks 
        // determin the largest network and it's max innovation number
        // and store that and the smaller network and it's max innovation number
        let (mut e, mut d) = (0.0, 0.0);
        let (mut w, mut wc) = (0.0, 0.0);
        let one_max = one.max_marker();
        let two_max = two.max_marker();
        let (big, small, small_innov) = if one_max > two_max { 
            (one, two, two_max)
        } else { 
            (two, one, one_max)
        };
        // iterate through the larger network 
        for (innov, edge) in big.edges.iter() {
            // check if it's a sharred innvation number
            if small.edges.contains_key(innov) {
                w += (edge.weight - small.edges.get(innov).unwrap().weight).abs();
                wc += 1.0;
                continue;
            }
            if innov > &small_innov {
                e += 1.0;
            } else {
                d += 1.0;
            }
        }
        // disjoint genes can be found within both networks unlike excess, so we still need to 
        // go through the smaller network and see if there are any disjoint genes in there as well
        for innov in small.edges.keys() {
            if !big.edges.contains_key(innov) {
                d += 1.0;
            }
        }
        // lock the env to get the comparing values from it  and make sure wc is greater than 0
        let wc = if wc == 0.0 { 1.0 } else { wc };
        let lock_env = (*env).read().unwrap();
        // return the distance between the two networks
        ((lock_env.c1.unwrap() * e) / big.edges.len() as f64) + ((lock_env.c2.unwrap() * d) / big.edges.len() as f64) + (lock_env.c3.unwrap() * (w / wc))
    }
}




/// Implement clone for the neat neural network in order to facilitate 
/// proper crossover and mutation for the network
impl Clone for Dense {
    fn clone(&self) -> Self {
        Dense {
            inputs: self.inputs.iter().map(|x| *x).collect(),
            outputs: self.outputs.iter().map(|x| *x).collect(),
            nodes: self.nodes.iter()
                .map(|(key, val)| {
                    let node = unsafe { (**val).clone() };
                    (*key, node.as_mut_ptr())
                })
                .collect(),
            edges: self.edges.iter()
                .map(|(key, val)| {
                    (*key, val.clone())
                })
                .collect(),
            layer_type: self.layer_type.clone(),
            activation: self.activation.clone()
        }
    }
}
/// Because the tree is made out of raw mutable pointers, if those pointers
/// are not dropped, there is a severe memory leak, like possibly gigs of
/// ram over only a few generations depending on the size of the generation
/// This drop implementation will recursivley drop all nodes in the tree 
impl Drop for Dense {
    fn drop(&mut self) { 
        unsafe {
            for (_, node) in self.nodes.iter() {
                ptr::drop_in_place(*node);
            }
        }
    }
}
/// These must be implemneted for the network or any type to be 
/// used within seperate threads. Because implementing the functions 
/// themselves is dangerious and unsafe and i'm not smart enough 
/// to do that from scratch, these "implmenetaions" will get rid 
/// of the error and realistically they don't need to be implemneted for the
/// program to work
unsafe impl Send for Dense {}
unsafe impl Sync for Dense {}
/// Implement partialeq for neat because if neat itself is to be used as a problem,
/// it must be able to compare one to another
impl PartialEq for Dense {
    fn eq(&self, other: &Self) -> bool {
        if self.edges.len() != other.edges.len() || self.nodes.len() != other.nodes.len() {
            return false;
        } 
        for (one, two) in self.edges.values().zip(other.edges.values()) {
            if one != two {
                return false;
            }
        }
        true
    }
}
/// Simple override of display for neat to debug a little cleaner 
impl fmt::Display for Dense {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            let address: u64 = mem::transmute(self);
            write!(f, "Dense=[{}, {}, {}]", address, self.nodes.len(), self.edges.len())
        }
    }
}

