use super::{Register, State, Transition, Value};
use itertools::Itertools;
use pathfinding::directed::astar::astar;
use std::cmp::min;

// TODO: Caches results using normalized version of the problem.

impl State {
    pub(crate) fn transition_to(&self, goal: &Self) -> Vec<Transition> {
        assert!(self.reachable(goal));

        // Find the optimal transition using pathfinder's A*
        let mut nodes_explored = 0;
        let (path, cost) = astar(
            self,
            |n| {
                // println!(
                //     "Exploring from (node {}) (min_dist {}):\n{}",
                //     nodes_explored,
                //     n.min_distance(goal),
                //     n
                // );
                n.useful_transitions(goal)
                    .into_iter()
                    .filter_map(|t| {
                        nodes_explored += 1;
                        // TODO: lazily compute next state?
                        let mut new_state = n.clone();
                        t.apply(&mut new_state);
                        if new_state.is_valid() && new_state.reachable(goal) {
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

        // Test admisability criterion along path
        // #[cfg(debug)]
        // test::test_admisability(self, goal, &result);

        result
    }

    fn register_set_cost(&self, dest: Option<Register>, value: Value) -> usize {
        use Transition::*;
        use Value::*;
        // No goal
        if !value.is_specified() {
            return 0;
        }
        // TODO: Copy does not take swaps into account

        // Ignore References
        // TODO: Copy for references if not in place
        if let Reference { .. } = value {
            // Assume the reference is available somewhere. If wo do Alloc,
            // we need to subtract this cost.
            if let Some(dest) = dest {
                if let Reference { .. } = self.get_register(dest) {
                    // Assume it is the right one and already in place.
                    return 0;
                } else {
                    return min(
                        Copy {
                            // TODO: It would be more accurate to use `dest` here,
                            // but that would be hard to undo when this thing gets
                            // replaced by an Alloc.
                            dest:   Register(0),
                            source: Register(0),
                        }
                        .cost(),
                        Swap {
                            dest:   Register(0),
                            source: Register(0),
                        }
                        .cost(),
                    );
                }
            }
            return min(
                Copy {
                    dest:   Register(0),
                    source: Register(0),
                }
                .cost(),
                Swap {
                    dest:   Register(0),
                    source: Register(0),
                }
                .cost(),
            );
        }

        // Compute best among a few strategies
        let mut cost = usize::max_value();

        // Try copy from registers
        for source in (0..=15).map(Register) {
            if value == self.get_register(source) {
                cost = min(cost, match dest {
                    None => 0,
                    Some(dest) if dest == source => 0,
                    Some(dest) => min(Copy { dest, source }.cost(), Swap { dest, source }.cost()),
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

        assert_ne!(cost, usize::max_value());
        cost
    }

    pub(crate) fn min_distance(&self, goal: &Self) -> usize {
        use Transition::*;
        use Value::*;
        // Compute minimum distance by taking the sum of the minimum cost to set
        // each goal register from the current state.
        // Note: this is not a perfect minimum: for example two `Set`
        // transitions with identical `value` can be more expensive than
        // a `Set` followed by `Copy`.

        // Early exit with max distance if goal is unreachable.
        if !self.reachable(goal) {
            return usize::max_value();
        }

        let mut cost = 0;
        // let mut constructed: Set<Value> = Set::default();

        // TODO: Values only have to be Set or Read once, after that they can be Copy'd
        // A Copy is not always better though.

        // let get_cost = |dest, val| {
        // let construct_cost = ;
        // if constructed.contains(val) {
        // if let Some(dest) = dest {
        // min(construct_cost, Copy { dest, source }.cost())
        // } else {
        // 0
        // }
        // } else {
        // self.register_set_cost(dest, *goal);
        // }
        // };

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
            // Since Alloc is in place, we can undo one Copy
            alloc_cost -= min(
                Copy {
                    dest:   Register(0),
                    source: Register(0),
                }
                .cost(),
                Swap {
                    dest:   Register(0),
                    source: Register(0),
                }
                .cost(),
            );
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
                for (ours, goal) in ours.iter().zip(goal.iter()) {
                    if !goal.is_specified() || ours == goal {
                        // Good as is
                        continue;
                    }
                    change_cost += write_cost + self.register_set_cost(None, *goal);
                }
                alloc_cost = min(alloc_cost, change_cost);
            }

            // Add to total cost
            cost += alloc_cost;
        }

        // TODO: Drops

        cost
    }

    fn useful_transitions(&self, goal: &Self) -> Vec<Transition> {
        let mut result = Vec::default();
        // TODO: Filter out invalid transitions (which would lose references)
        // TODO: No need to enumerate all cases of writing to an Unspecified, one
        // should be sufficient.
        // TODO: Nearly always no need to write to a place that is already correct.

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

        // Drop an existing reference
        for dest in (0..=15).map(Register) {
            if let Value::Reference { .. } = self.get_register(dest) {
                result.push(Transition::Drop { dest });
            }
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::{super::Allocation, *};
    use proptest::strategy::Strategy;

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
        let optimal_cost = optimal_path.iter().map(|t| t.cost()).sum::<usize>();
        test_admisability(&initial, &goal, &optimal_path);
        let path = initial.transition_to(&goal);
        let path_cost = optimal_path.iter().map(|t| t.cost()).sum::<usize>();
        assert_eq!(optimal_cost, path_cost);
    }

    /// Provided a known best bath, test heuristic admisability.
    fn test_admisability(initial: &State, goal: &State, path: &[Transition]) {
        println!("Initial:\n{}", initial);
        println!("Goal:\n{}", goal);

        // Reconstruct intermediate states
        let mut states = vec![initial.clone()];
        for ts in path {
            let mut next = states.last().unwrap().clone();
            ts.apply(&mut next);
            states.push(next);
            println!(" {:7}: {:?}", ts.cost(), ts);
        }
        assert!(states.last().unwrap().satisfies(&goal));

        // Check admisability: min_distance is always <= the real cost
        // <https://en.wikipedia.org/wiki/Admissible_heuristic>
        let mut overall_admisable = true;
        for start in (0..states.len()) {
            for end in (start..states.len()) {
                let heuristic = states[start].min_distance(&states[end]);
                let distance = path
                    .iter()
                    .skip(start)
                    .take(end - start)
                    .map(|t| t.cost())
                    .sum::<usize>();
                let admisable = heuristic <= distance;
                println!(
                    "{} - {}: {:7} {:7} {:5}",
                    start, end, heuristic, distance, admisable
                );
                overall_admisable &= admisable;
            }

            // Goal as target
            let heuristic = states[start].min_distance(&goal);
            let distance = path.iter().skip(start).map(|t| t.cost()).sum::<usize>();
            let admisable = heuristic <= distance;
            println!(
                "{} - G: {:7} {:7} {:5}",
                start, heuristic, distance, admisable
            );
            overall_admisable &= admisable;
        }
        assert!(overall_admisable);
    }

    // Check consistency
    // <https://en.wikipedia.org/wiki/Consistent_heuristic>
    // Take any pair of states, iterate all neighbouring states.
    fn test_consistency(initial: &State, goal: &State) {
        println!("Initial:\n{}", initial);
        println!("Goal:\n{}", goal);
        let mindist = initial.min_distance(goal);
        let mut overal_consistent = true;
        println!("Heuristic distance: {}", mindist);
        for ts in initial.useful_transitions(goal) {
            let mut neighbor = initial.clone();
            ts.apply(&mut neighbor);
            let cost = ts.cost();
            let dist = neighbor.min_distance(goal);
            let consistent = (cost + dist) >= mindist;
            println!(" {:5} {:7} {:7}: {:?}", consistent, ts.cost(), dist, ts);
            overal_consistent &= consistent;
        }
        assert!(overal_consistent);
    }

    #[test]
    fn test_basic() {
        use Transition::*;
        use Value::*;
        let mut initial = State::default();
        initial.registers[0] = Symbol(1);
        initial.registers[1] = Symbol(2);
        initial.registers[2] = Symbol(3);
        let mut goal = State::default();
        goal.registers[0] = Reference {
            index:  0,
            offset: 0,
        };
        goal.registers[1] = Symbol(3);
        goal.registers[2] = Literal(3);
        goal.allocations
            .push(Allocation(vec![Symbol(1), Symbol(2)]));

        let path = initial.transition_to(&goal);
        test_admisability(&initial, &goal, &path);
        test_consistency(&initial, &goal);
    }

    #[test]
    fn test_basic2() {
        use Transition::*;
        use Value::*;
        let mut initial = State::default();
        initial.registers[0] = Symbol(0);
        initial.registers[1] = Symbol(1);
        initial.registers[2] = Symbol(2);
        initial.registers[3] = Symbol(3);
        initial.registers[4] = Symbol(4);

        let mut goal = State::default();
        goal.registers[0] = Literal(0x0000000000100058);
        goal.registers[1] = Symbol(1);
        goal.registers[2] = Symbol(2);
        goal.registers[3] = Reference {
            index:  0,
            offset: 0,
        };
        goal.allocations.push(Allocation(vec![
            Literal(0x0000000000100058),
            Symbol(3),
            Symbol(4),
        ]));

        let path = initial.transition_to(&goal);
        test_admisability(&initial, &goal, &path);
        test_consistency(&initial, &goal);
    }
}
