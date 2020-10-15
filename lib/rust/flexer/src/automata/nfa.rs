//! The structure for defining non-deterministic finite automata.

use crate::automata::alphabet;
use crate::automata::pattern::Pattern;
use crate::automata::state::State;
use crate::automata::state::Transition;
use crate::automata::state;
use crate::automata::symbol::Symbol;
use crate::automata::data::matrix::Matrix;

use itertools::Itertools;
use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use crate::prelude::*;



// =========================================
// === Non-Deterministic Finite Automata ===
// =========================================

/// A state identifier based on a set of states.
///
/// This is used during the NFA -> DFA transformation, where multiple states can merge together due
/// to the collapsing of epsilon transitions.
pub type StateSetId = BTreeSet<state::Identifier>;

/// The definition of a [NFA](https://en.wikipedia.org/wiki/Nondeterministic_finite_automaton) for a
/// given set of symbols, states, and transitions (specifically a NFA with ε-moves).
///
/// A NFA is a finite state automaton that accepts or rejects a given sequence of symbols. In
/// contrast with a DFA, the NFA may transition between states _without_ reading any new symbol
/// through use of
/// [epsilon links](https://en.wikipedia.org/wiki/Nondeterministic_finite_automaton#NFA_with_%CE%B5-moves).
///
/// ```text
///  ┌───┐  'N'  ┌───┐    ┌───┐  'F'  ┌───┐    ┌───┐  'A'  ┌───┐
///  │ 0 │ ----> │ 1 │ -> │ 2 │ ----> │ 3 │ -> │ 3 │ ----> │ 3 │
///  └───┘       └───┘ ε  └───┘       └───┘ ε  └───┘       └───┘
/// ```
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub struct NFA {
    /// A set of disjoint intervals over the input alphabet.
    pub alphabet_segmentation:alphabet::Segmentation,
    /// A set of named NFA states, with (epsilon) transitions.
    pub states:Vec<State>,
}

impl NFA {
    /// Adds a new state to the NFA and returns its identifier.
    pub fn new_state(&mut self) -> state::Identifier {
        let id = self.states.len();
        self.states.push(State::default());
        state::Identifier{id}
    }

    /// Creates an epsilon transition between two states.
    ///
    /// Whenever the automaton happens to be in `source` state it can immediately transition to the
    /// `target` state. It is, however, not _required_ to do so.
    pub fn connect(&mut self, source:state::Identifier, target:state::Identifier) {
        self.states[source.id].add_epsilon_link(target);
    }

    /// Creates an ordinary transition for a range of symbols.
    ///
    /// If any symbol from such range happens to be the input when the automaton is in the `source`
    /// state, it will immediately transition to the `target` state.
    pub fn connect_via
    ( &mut self
    , source       : state::Identifier
    , target_state : state::Identifier
    , symbols      : &RangeInclusive<Symbol>
    ) {
        self.alphabet_segmentation.insert(symbols.clone());
        self.states[source.id].add_link(Transition::new(symbols.clone(),target_state));
    }

    // TODO [AA] Tests for these results.
    /// Transforms a pattern to an NFA using the algorithm described
    /// [here](https://www.youtube.com/watch?v=RYNN-tb9WxI).
    /// The asymptotic complexity is linear in number of symbols.
    pub fn new_pattern(&mut self, source:state::Identifier, pattern:&Pattern) -> state::Identifier {
        let current = self.new_state();
        self.connect(source,current);
        match pattern {
            Pattern::Range(range) => {
                let state = self.new_state();
                self.connect_via(current,state,range);
                state
            },
            Pattern::Many(body) => {
                let s1 = self.new_state();
                let s2 = self.new_pattern(s1,body);
                let s3 = self.new_state();
                self.connect(current,s1);
                self.connect(current,s3);
                self.connect(s2,s3);
                self.connect(s3,s1);
                s3
            },
            Pattern::Seq(patterns) => {
                patterns.iter().fold(current,|s,pat| self.new_pattern(s,pat))
            },
            Pattern::Or(patterns) => {
                let states = patterns.iter().map(|pat| self.new_pattern(current,pat)).collect_vec();
                let end    = self.new_state();
                for state in states {
                    self.connect(state,end);
                }
                end
            },
            Pattern::Always => current,
            Pattern::Never  => self.new_state(),
        }
    }

    /// Merges states that are connected by epsilon links, using an algorithm based on the one shown
    /// [here](https://www.youtube.com/watch?v=taClnxU-nao).
    pub fn eps_matrix(&self) -> Vec<StateSetId> {
        fn fill_eps_matrix
        ( nfa      : &NFA
        , states   : &mut Vec<StateSetId>
        , visited  : &mut Vec<bool>
        , state    : state::Identifier
        ) {
            let mut state_set = StateSetId::new();
            visited[state.id] = true;
            state_set.insert(state);
            for &target in nfa.states[state.id].epsilon_links() {
                if !visited[target.id] {
                    fill_eps_matrix(nfa,states,visited,target);
                }
                state_set.insert(target);
                state_set.extend(states[target.id].iter());
            }
            states[state.id] = state_set;
        }

        let mut states = vec![StateSetId::new(); self.states.len()];
        for id in 0..self.states.len() {
            let mut visited = vec![false; states.len()];
            fill_eps_matrix(self,&mut states,&mut visited,state::Identifier{id});
        }
        states
    }

    /// Computes a transition matrix `(state, symbol) => state` for the NFA, ignoring epsilon links.
    pub fn nfa_matrix(&self) -> Matrix<state::Identifier> {
        let mut matrix = Matrix::new(self.states.len(),self.alphabet_segmentation.len());

        for (state_ix, source) in self.states.iter().enumerate() {
            let targets = source.targets(&self.alphabet_segmentation);
            for (voc_ix, &target) in targets.iter().enumerate() {
                matrix[(state_ix,voc_ix)] = target;
            }
        }
        matrix
    }
}



// ===========
// == Tests ==
// ===========

#[cfg(test)]
pub mod tests {
    use super::*;

    // TODO [AA] Simple single-element rules.
    // TODO [AA] More-complex chained rules.
    // TODO [AA] Test the basic cases of patterns.
    // TODO [AA] Second group from flexer test

    #[test]
    fn nfa_pattern_range() {

    }

    #[test]
    fn nfa_pattern_or() {

    }

    #[test]
    fn nfa_pattern_seq() {

    }

    #[test]
    fn nfa_pattern_many() {

    }

    #[test]
    fn nfa_pattern_always() {

    }

    #[test]
    fn nfa_pattern_never() {

    }

    #[test]
    fn nfa_simple_rules() {

    }

    #[test]
    fn nfa_complex_rules() {

    }

}
