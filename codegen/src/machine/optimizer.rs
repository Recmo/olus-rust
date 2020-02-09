use super::{Register, State, Transition, Value};
use itertools::Itertools;
use pathfinding::directed::astar::astar;
use std::cmp::min;

// TODO: Caches results using normalized version of the problem.

struct TransitionIter {
    trans: Vec<Transition>,
    index: usize,
}

impl State {
    fn transition_to(&self, goal: &Self) -> Vec<Transition> {
        assert!(self.reachable(goal));

        // Find the optimal transition using pathfinder's A*
        let mut nodes_explored = 0;
        let (path, cost) = astar(
            self,
            |n| {
                println!(
                    "Exploring from (node {}) (min_dist {}):\n{}",
                    nodes_explored,
                    n.min_distance(goal),
                    n
                );
                n.useful_transitions(goal)
                    .filter_map(|t| {
                        nodes_explored += 1;
                        // TODO: lazily compute next state?
                        let mut new_state = n.clone();
                        t.apply(&mut new_state);
                        if new_state.is_valid() {
                            Some((new_state, t.cost()))
                        } else {
                            None
                        }
                    })
                    // TODO: Don't allocate
                    .collect::<Vec<_>>()
            },
            |n| n.min_distance(goal),
            |n| n.satisfies(goal),
        )
        .expect("Could not find valid transition path");
        println!("Nodes explored: {}", nodes_explored);
        println!("Cost: {}", cost);

        // Pathfinder gives a list of nodes visited, not the path taken.
        // So take all the pairs of nodes and find the best transition
        // between them.
        let mut result = Vec::default();
        for (from, to) in path.iter().tuple_windows() {
            let mut cost = usize::max_value();
            let mut best = None;
            for transition in from.useful_transitions(goal) {
                let mut dest = from.clone();
                transition.apply(&mut dest);
                if dest == *to && transition.cost() < cost {
                    cost = transition.cost();
                    best = Some(transition);
                }
            }
            result.push(best.expect("Could not reproduce path"));
        }
        result
    }

    fn register_set_cost(&self, dest: Option<Register>, value: Value) -> usize {
        use Transition::*;
        use Value::*;
        // No goal
        if !value.is_specified() {
            return 0;
        }

        // Ignore References
        if let Reference { .. } = value {
            return 0;
        }

        // Compute best among a few strategies
        let mut cost = usize::max_value();

        // Try copy from registers
        for source in (0..=15).map(Register) {
            if value == self.get_register(source) {
                cost = min(cost, match dest {
                    None => 0,
                    Some(dest) if dest == source => 0,
                    Some(dest) => Copy { dest, source }.cost(),
                });
                if cost == 0 {
                    return cost;
                }
            }
        }
        let dest = dest.unwrap_or(Register(0));

        // Try literals
        if let Literal(value) = value {
            cost = min(cost, Set { dest, value }.cost());
        }

        // Try copy from allocations
        let read_cost = Read {
            dest,
            source: Register(0),
            offset: 0,
        }
        .cost();
        if cost <= read_cost {
            return cost;
        }
        for alloc in &self.allocations {
            for alloc_val in alloc {
                if *alloc_val == value {
                    return read_cost;
                }
            }
        }
        cost
    }

    fn min_distance(&self, goal: &Self) -> usize {
        use Transition::*;
        use Value::*;
        // Compute minimum distance by taking the sum of the minimum cost to set
        // each goal register from the current state.
        // Note: this is not a perfect minimum: for example two `Set`
        // transitions with identical `value` can be more expensive than
        // a `Set` followed by `Copy`.
        let mut cost = 0;

        // TODO: Function to return minimal cost of constructing a value in a given
        // register using Copy Set or Read.

        // Registers
        for (i, (ours, goal)) in self.registers.iter().zip(goal.registers.iter()).enumerate() {
            cost += self.register_set_cost(Some(Register(i as u8)), *goal);
        }
        // TODO: Flags

        // Allocations
        let write_cost = Write {
            dest:   Register(0),
            offset: 0,
            source: Register(0),
        }
        .cost();
        for goal in &goal.allocations {
            // Compute the cost of constructing it from scratch
            let mut alloc_cost = Alloc {
                dest: Register(0),
                size: goal.len(),
            }
            .cost();
            for goal in goal.iter() {
                if goal.is_specified() {
                    alloc_cost += write_cost + self.register_set_cost(None, *goal);
                }
            }

            // See if we can change an existing allocation
            for ours in &self.allocations {
                if ours.len() != goal.len() {
                    continue;
                }
                let mut change_cost = 0;
                for (ours, goal) in ours.iter().zip(ours.iter()) {
                    if !goal.is_specified() || ours == goal {
                        // Good as is
                        continue;
                    }
                    change_cost += write_cost + self.register_set_cost(None, *goal);
                }
                alloc_cost = min(alloc_cost, change_cost);
            }

            cost += alloc_cost;
        }
        cost
    }

    fn useful_transitions(&self, goal: &Self) -> TransitionIter {
        let mut result = Vec::default();
        // TODO: Filter out invalid transitions (which would lose references)
        // TODO: No need to enumerate all cases of writing to an Unspecified, one
        // should be sufficient.

        // Generate Set transitions for each goal literal and register.
        for value in goal.literals().into_iter() {
            for dest in (0..=15).map(Register) {
                result.push(Transition::Set { dest, value });
            }
        }

        // Copy and swap registers around
        for source in (0..=15).map(Register) {
            // No point in copying from unspecified regs
            if !self.get_register(source).is_specified() {
                continue;
            }

            // Generate moves and swaps between registers
            for dest in (0..=15).map(Register) {
                // Copy to any reg
                if source != dest {
                    result.push(Transition::Copy { dest, source });
                }
                // Swap two regs
                if source < dest && self.get_register(dest).is_specified() {
                    result.push(Transition::Swap { dest, source });
                }
            }

            // Generate reads and writes
            if let Value::Reference {
                index,
                offset: base_offset,
            } = self.get_register(source)
            {
                let values = &self.allocations[index];
                for offset in (0..values.len()).map(|n| (n as isize) - base_offset) {
                    for dest in (0..=15).map(Register) {
                        // Read if there is something there
                        if self.get_reference(source, offset).unwrap().is_specified() {
                            result.push(Transition::Read {
                                dest,
                                source,
                                offset,
                            });
                        }

                        // Writes have source and dest flipped
                        if self.get_register(dest).is_specified() {
                            result.push(Transition::Write {
                                dest: source,
                                offset,
                                source: dest,
                            });
                        }
                    }
                }
            }
        }

        // Allocate for goal sizes
        for size in goal.alloc_sizes().into_iter() {
            for dest in (0..=15).map(Register) {
                result.push(Transition::Alloc { dest, size });
            }
        }

        TransitionIter {
            trans: result,
            index: 0,
        }
    }
}

impl Iterator for TransitionIter {
    type Item = Transition;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.trans.get(self.index);
        self.index += 1;
        value.map(|a| *a)
    }
}

#[cfg(test)]
mod test {
    use super::{super::Allocation, *};

    #[test]
    fn test_min_distance() {
        use Transition::*;
        use Value::*;
        let mut initial = State::default();
        initial.registers[0] = Symbol(5);
        let mut goal = State::default();
        goal.registers[0] = Literal(3);
        goal.registers[1] = Reference {
            index:  0,
            offset: 0,
        };
        goal.allocations.push(Allocation(vec![Symbol(5)]));

        let optimal_path = vec![
            Alloc {
                dest: Register(1),
                size: 1,
            },
            Write {
                dest:   Register(1),
                offset: 0,
                source: Register(0),
            },
            Set {
                dest:  Register(0),
                value: 3,
            },
        ];

        // assert_eq!(
        // initial.min_distance(&goal),
        // optimal_path
        // .iter()
        // .map(Transition::cost)
        // .map(|a| dbg!(a))
        // .sum()
        // );

        let mut state1 = initial.clone();
        optimal_path[0].apply(&mut state1);
        let mut state2 = state1.clone();
        optimal_path[1].apply(&mut state2);

        println!("Initial:\n{}", initial);
        println!("State 1:\n{}", state1);
        println!("State 2:\n{}", state2);
        println!("Goal:\n{}", goal);

        dbg!(initial.min_distance(&state1));
        dbg!(state1.min_distance(&state2));
        dbg!(state2.min_distance(&goal));

        dbg!(initial.min_distance(&goal));
        dbg!(state1.min_distance(&goal));
        dbg!(state2.min_distance(&goal));

        println!("Cost estimate: {}", initial.min_distance(&goal));
    }

    #[test]
    fn test_basic() {
        use Transition::*;
        use Value::*;
        let mut initial = State::default();
        initial.registers[0] = Symbol(5);
        let mut goal = State::default();
        goal.registers[0] = Literal(3);
        goal.registers[1] = Reference {
            index:  0,
            offset: 0,
        };
        goal.allocations.push(Allocation(vec![Symbol(5)]));
        println!("Initial:\n{}", initial);
        println!("Goal:\n{}", goal);
        println!("Cost estimate: {}", initial.min_distance(&goal));

        let path = initial.transition_to(&goal);
        println!("Path:\n{:?}", path);
    }
}
