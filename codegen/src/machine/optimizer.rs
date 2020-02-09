use super::{Register, State, Transition, Value};
use itertools::Itertools;
use pathfinding::directed::astar::astar;

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

    fn register_set_cost(&self, reg: Register, value: Value) -> usize {
        use Transition::*;
        use Value::*;
        if self.get_register(reg) == value {
            return 0;
        }
        match value {
            Unspecified => 0,
            // Copy from existing reg may be cheaper, should `min` over the options.
            Literal(n) => {
                Set {
                    dest:  reg,
                    value: n,
                }
                .cost()
            }
            Symbol(n) => {
                Copy {
                    dest:   reg,
                    source: Register(0), // Assume a cheap register, could also be read
                }
                .cost()
            }
            Reference { .. } => {
                // Allocations are computed seperately
                0
            }
        }
    }

    fn min_distance(&self, goal: &Self) -> usize {
        // Compute minimum distance by taking the sum of the minimum cost to set
        // each goal register from the current state.
        // Note: this is not a perfect minimum: for example two `Set`
        // transitions with identical `value` can be more expensive than
        // a `Set` followed by `Copy`.

        // TODO: Delta the allocations sizes and account for Alloc + Drop
        let allocs = goal
            .allocations
            .len()
            .saturating_sub(self.allocations.len());
        let allocs = allocs
            * Transition::Alloc {
                dest: Register(0),
                size: 1,
            }
            .cost();
        dbg!(allocs);
        allocs
            + goal
                .into_iter()
                .enumerate()
                .map(|(i, value)| {
                    {
                        // Check if any allocation already has this value, if so
                        // we return zero assuming it is already set
                        // TODO: Need to align allocations.

                        if !value.is_specified() {
                            0
                        } else if i < 16 {
                            self.register_set_cost(Register(i as u8), *value)
                        } else if i < 23 {
                            // TODO: Flags
                            0
                        } else {
                            (0..=15)
                                .map(Register)
                                .map(|source| {
                                    self.register_set_cost(source, *value)
                                        + Transition::Write {
                                            // TODO: RSP may be cheaper
                                            dest: Register(0),
                                            offset: 0,
                                            source,
                                        }
                                        .cost()
                                })
                                .min()
                                .unwrap()
                        }
                    }
                })
                .map(|a| dbg!(a))
                .sum::<usize>()
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

        // dbg!(initial.min_distance(&state1));
        // dbg!(state1.min_distance(&state2));
        // dbg!(state2.min_distance(&goal));
        //
        // dbg!(initial.min_distance(&goal));
        // dbg!(state1.min_distance(&goal));
        dbg!(state2.min_distance(&goal));

        // println!("Cost estimate: {}", initial.min_distance(&goal));
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
