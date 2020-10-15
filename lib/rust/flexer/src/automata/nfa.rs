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

    // === Test Utilities ===

    #[allow(missing_docs)]
    #[derive(Clone,Debug,Default,PartialEq)]
    pub struct NfaTest {
        pub nfa               : NFA,
        pub start_state_id    : state::Identifier,
        pub pattern_state_ids : Vec<state::Identifier>,
        pub end_state_id      : state::Identifier,
    }
    #[allow(missing_docs)]
    impl NfaTest {
        pub fn make(patterns:Vec<Pattern>) -> Self {
            let mut nfa          = NFA::default();
            let start_state_id   = nfa.new_state();
            let mut pattern_state_ids = vec![];
            let end_state_id     = nfa.new_state();
            for pattern in patterns {
                let id = nfa.new_pattern(start_state_id,&pattern);
                pattern_state_ids.push(id);
                nfa.connect(id,end_state_id);
            }
            Self{nfa,start_state_id,pattern_state_ids,end_state_id}
        }

        pub fn id(id:usize) -> state::Identifier {
            state::Identifier::new(id)
        }

        pub fn has_transition(&self, trigger:RangeInclusive<Symbol>, target:state::Identifier) -> bool {
            self.states.iter().fold(false,|l,r| {
                l || r.links().iter().find(|transition | {
                    (transition.symbols == trigger) && transition.target_state == target
                }).is_some()
            })
        }

        pub fn has_epsilon(&self, from:state::Identifier, to:state::Identifier) -> bool {
            self.states.iter().enumerate().fold(false,|l,(ix,r)| {
                let state_has = ix == from.id && r.epsilon_links().iter().find(|ident| {
                    **ident == to
                }).is_some();
                l || state_has
            })
        }
    }
    impl Deref for NfaTest {
        type Target = NFA;

        fn deref(&self) -> &Self::Target {
            &self.nfa
        }
    }


    // === The Automata ===

    pub fn pattern_range() -> NfaTest {
        let pattern = Pattern::range('a'..='z');
        NfaTest::make(vec![pattern])
    }

    pub fn pattern_or() -> NfaTest {
        let pattern = Pattern::char('a') | Pattern::char('d');
        NfaTest::make(vec![pattern])
    }

    pub fn pattern_seq() -> NfaTest {
        let pattern = Pattern::char('a') >> Pattern::char('d');
        NfaTest::make(vec![pattern])
    }

    pub fn pattern_many() -> NfaTest {
        let pattern = Pattern::char('a').many();
        NfaTest::make(vec![pattern])
    }

    pub fn pattern_always() -> NfaTest {
        let pattern = Pattern::always();
        NfaTest::make(vec![pattern])
    }

    pub fn pattern_never() -> NfaTest {
        let pattern = Pattern::never();
        NfaTest::make(vec![pattern])
    }

    pub fn simple_rules() -> NfaTest {
        let a   = Pattern::char('a');
        let b   = Pattern::char('b');
        let ab  = &a >> &b;
        NfaTest::make(vec![a,ab])
    }

    pub fn complex_rules() -> NfaTest {
        let a_word        = Pattern::char('a').many1();
        let b_word        = Pattern::char('b').many1();
        let space         = Pattern::char(' ');
        let spaced_a_word = &space >> &a_word;
        let spaced_b_word = &space >> &b_word;
        let any           = Pattern::any();
        let end           = Pattern::eof();
        NfaTest::make(vec![spaced_a_word,spaced_b_word,end,any])
    }


    // === The Tests ===

    #[test]
    fn nfa_pattern_range() {
        let nfa = pattern_range();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(97)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(123)));
        assert_eq!(nfa.states.len(),4);
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(2)));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],nfa.end_state_id));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('z'),nfa.pattern_state_ids[0]));
    }

    #[test]
    fn nfa_pattern_or() {
        let nfa = pattern_or();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(97)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(98)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(100)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(101)));
        assert_eq!(nfa.states.len(),8);
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(2)));
        assert!(nfa.has_epsilon(NfaTest::id(2),NfaTest::id(3)));
        assert!(nfa.has_epsilon(NfaTest::id(2),NfaTest::id(5)));
        assert!(nfa.has_epsilon(NfaTest::id(6),nfa.pattern_state_ids[0]));
        assert!(nfa.has_epsilon(NfaTest::id(4),nfa.pattern_state_ids[0]));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],nfa.end_state_id));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),NfaTest::id(4)));
        assert!(nfa.has_transition(Symbol::from('d')..=Symbol::from('d'),NfaTest::id(6)));
    }

    #[test]
    fn nfa_pattern_seq() {
        let nfa = pattern_seq();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(97)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(98)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(100)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(101)));
        assert_eq!(nfa.states.len(),7);
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(2)));
        assert!(nfa.has_epsilon(NfaTest::id(2),NfaTest::id(3)));
        assert!(nfa.has_epsilon(NfaTest::id(4),NfaTest::id(5)));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],nfa.end_state_id));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),NfaTest::id(4)));
        assert!(nfa.has_transition(Symbol::from('d')..=Symbol::from('d'),NfaTest::id(6)));
    }

    #[test]
    fn nfa_pattern_many() {
        let nfa = pattern_many();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(97)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(98)));
        assert_eq!(nfa.states.len(),7);
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(2)));
        assert!(nfa.has_epsilon(NfaTest::id(2),NfaTest::id(3)));
        assert!(nfa.has_epsilon(NfaTest::id(3),NfaTest::id(4)));
        assert!(nfa.has_epsilon(NfaTest::id(5),nfa.pattern_state_ids[0]));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],NfaTest::id(3)));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],nfa.end_state_id));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),NfaTest::id(5)));
    }

    #[test]
    fn nfa_pattern_always() {
        let nfa = pattern_always();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert_eq!(nfa.states.len(),3);
        assert!(nfa.has_epsilon(nfa.start_state_id,nfa.pattern_state_ids[0]));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],nfa.end_state_id));
    }

    #[test]
    fn nfa_pattern_never() {
        let nfa = pattern_never();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert_eq!(nfa.states.len(),4);
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(2)));
        assert!(nfa.has_epsilon(NfaTest::id(3),nfa.end_state_id));
    }

    #[test]
    fn nfa_simple_rules() {
        let nfa = simple_rules();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(97)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(98)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(99)));
        assert_eq!(nfa.states.len(),9);
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(2)));
        assert!(nfa.has_epsilon(nfa.start_state_id,NfaTest::id(4)));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[0],nfa.end_state_id));
        assert!(nfa.has_epsilon(NfaTest::id(4),NfaTest::id(5)));
        assert!(nfa.has_epsilon(NfaTest::id(6),NfaTest::id(7)));
        assert!(nfa.has_epsilon(nfa.pattern_state_ids[1],nfa.end_state_id));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),nfa.pattern_state_ids[0]));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),NfaTest::id(6)));
        assert!(nfa.has_transition(Symbol::from('b')..=Symbol::from('b'),nfa.pattern_state_ids[1]));
    }

    #[test]
    fn nfa_complex_rules() {
        let nfa = complex_rules();

        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(0)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(32)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(33)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(97)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(98)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::from(99)));
        assert!(nfa.alphabet_segmentation.divisions().contains(&Symbol::EOF_CODE));
        assert_eq!(nfa.states.len(),26);
        assert!(nfa.has_transition(Symbol::from(' ')..=Symbol::from(' '),NfaTest::id(4)));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),NfaTest::id(6)));
        assert!(nfa.has_transition(Symbol::from('a')..=Symbol::from('a'),NfaTest::id(10)));
        assert!(nfa.has_transition(Symbol::from(' ')..=Symbol::from(' '),NfaTest::id(14)));
        assert!(nfa.has_transition(Symbol::from('b')..=Symbol::from('b'),NfaTest::id(16)));
        assert!(nfa.has_transition(Symbol::from('b')..=Symbol::from('b'),NfaTest::id(20)));
        assert!(nfa.has_transition(Symbol::EOF_CODE..=Symbol::EOF_CODE,nfa.pattern_state_ids[2]));
        assert!(nfa.has_transition(Symbol::NULL..=Symbol::EOF_CODE,nfa.pattern_state_ids[3]));
    }
}
