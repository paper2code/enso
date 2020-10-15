//! The structure for defining deterministic finite automata.

use crate::automata::alphabet;
use crate::automata::nfa::NFA;
use crate::automata::state;
use crate::automata::data::matrix::Matrix;

use std::collections::BTreeSet;
use std::collections::HashMap;

use crate::prelude::*;



// =============
// === Types ===
// =============

/// A state identifier based on a set of states.
///
/// This is used during the NFA -> DFA transformation, where multiple states can merge together due
/// to the collapsing of epsilon transitions.
pub type StateSetId = BTreeSet<state::Identifier>;



// =====================================
// === Deterministic Finite Automata ===
// =====================================

/// The definition of a [DFA](https://en.wikipedia.org/wiki/Deterministic_finite_automaton) for a
/// given set of symbols, states, and transitions.
///
/// A DFA is a finite state automaton that accepts or rejects a given sequence of symbols by
/// executing on a sequence of states _uniquely_ determined by the sequence of input symbols.
///
/// ```text
///  ┌───┐  'D'  ┌───┐  'F'  ┌───┐  'A'  ┌───┐
///  │ 0 │ ----> │ 1 │ ----> │ 2 │ ----> │ 3 │
///  └───┘       └───┘       └───┘       └───┘
/// ```
#[derive(Clone,Debug,Default,Eq,PartialEq)]
pub struct DFA {
    /// A set of disjoint intervals over the allowable input alphabet.
    pub alphabet_segmentation:alphabet::Segmentation,
    /// The transition matrix for the DFA.
    ///
    /// It represents a function of type `(state, symbol) -> state`, returning the identifier for
    /// the new state.
    ///
    /// For example, the transition matrix for an automaton that accepts the language
    /// `{"A" | "B"}*"` would appear as follows, with `-` denoting
    /// [the invalid state](state::Identifier::INVALID). The leftmost column encodes the input
    /// state, while the topmost row encodes the input symbols.
    ///
    /// |   | A | B |
    /// |:-:|:-:|:-:|
    /// | 0 | 1 | - |
    /// | 1 | - | 0 |
    ///
    pub links:Matrix<state::Identifier>,
    /// A collection of callbacks for each state (indexable in order)
    pub callbacks:Vec<Option<RuleExecutable>>,
}

impl DFA {
    /// Check whether the DFA has a rule for the target state.
    ///
    /// This method should only be used in generated code, where its invariants are already checked.
    pub fn has_rule_for(&self, target_state:state::Identifier) -> bool {
        let callback = self.callbacks.get(target_state.id);
        callback.is_some() && callback.unwrap().is_some()
    }
}


// === Trait Impls ===

impl From<&NFA> for DFA {

    /// Transforms an NFA into a DFA, based on the algorithm described
    /// [here](https://www.youtube.com/watch?v=taClnxU-nao).
    /// The asymptotic complexity is quadratic in number of states.
    fn from(nfa:&NFA) -> Self {
        let     nfa_mat     = nfa.nfa_matrix();
        let     eps_mat     = nfa.eps_matrix();
        let mut dfa_mat     = Matrix::new(0,nfa.alphabet_segmentation.len());
        let mut dfa_eps_ixs = Vec::<StateSetId>::new();
        let mut dfa_eps_map = HashMap::<StateSetId,state::Identifier>::new();

        dfa_eps_ixs.push(eps_mat[0].clone());
        dfa_eps_map.insert(eps_mat[0].clone(),state::Identifier::from(0));

        let mut i = 0;
        while i < dfa_eps_ixs.len()  {
            dfa_mat.new_row();
            for voc_ix in 0..nfa.alphabet_segmentation.len() {
                let mut eps_set = StateSetId::new();
                for &eps_ix in &dfa_eps_ixs[i] {
                    let tgt = nfa_mat[(eps_ix.id,voc_ix)];
                    if tgt != state::Identifier::INVALID {
                        eps_set.extend(eps_mat[tgt.id].iter());
                    }
                }
                if !eps_set.is_empty() {
                    dfa_mat[(i,voc_ix)] = match dfa_eps_map.get(&eps_set) {
                        Some(&id) => id,
                        None => {
                            let id = state::Identifier::new(dfa_eps_ixs.len());
                            dfa_eps_ixs.push(eps_set.clone());
                            dfa_eps_map.insert(eps_set,id);
                            id
                        },
                    };
                }
            }
            i += 1;
        }

        let mut callbacks = vec![None; dfa_eps_ixs.len()];
        let     priority  = dfa_eps_ixs.len();
        for (dfa_ix, epss) in dfa_eps_ixs.into_iter().enumerate() {
            let has_name = |&key:&state::Identifier| nfa.states[key.id].name().is_some();
            if let Some(eps) = epss.into_iter().find(has_name) {
                let code          = nfa.states[eps.id].name().as_ref().cloned().unwrap();
                callbacks[dfa_ix] = Some(RuleExecutable {code,priority});
            }
        }

        let alphabet_segmentation = nfa.alphabet_segmentation.clone();
        let links = dfa_mat;

        DFA{alphabet_segmentation,links,callbacks}
    }
}

impl From<Vec<Vec<usize>>> for Matrix<state::Identifier> {
    fn from(input:Vec<Vec<usize>>) -> Self {
        let rows        = input.len();
        let columns     = if rows == 0 {0} else {input[0].len()};
        let mut matrix  = Self::new(rows,columns);
        for row in 0..rows {
            for column in 0..columns {
                matrix[(row,column)] = state::Identifier::from(input[row][column]);
            }
        }
        matrix
    }
}



// ================
// === Callback ===
// ================

/// The callback associated with an arbitrary state of a finite automaton.
///
/// It contains the rust code that is intended to be executed after encountering a
/// [`pattern`](super::pattern::Pattern) that causes the associated state transition. This pattern
/// is declared in [`Rule.pattern`](crate::group::rule::Rule::pattern).
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RuleExecutable {
    /// A description of the priority with which the callback is constructed during codegen.
    pub priority:usize,
    /// The rust code that will be executed when running this callback.
    pub code:String,
}

impl RuleExecutable {
    /// Creates a new rule executable with the provided `priority` and `code`.
    pub fn new(priority:usize, code_str:impl Into<String>) -> RuleExecutable {
        let code = code_str.into();
        RuleExecutable{priority,code}
    }
}



// =============
// === Tests ===
// =============

#[cfg(test)]
pub mod tests {
    use crate::automata::state;

    use super::*;

    // TODO [AA] Conversion tests from the nfa automata
    // TODO [AA] Check that the alphabet stays the same

    const INVALID:usize = state::Identifier::INVALID.id;

    /// DFA automata that accepts newline '\n'.
    pub fn newline() -> DFA {
        DFA {
            alphabet_segmentation:alphabet::Segmentation::from_divisions(&[10,11]),
            links:Matrix::from(vec![vec![INVALID,1,INVALID], vec![INVALID,INVALID,INVALID]]),
            callbacks:vec![
                None,
                Some(RuleExecutable {priority:2, code:"group_0_rule_0".into()}),
            ],
        }
    }

    /// DFA automata that accepts any letter a..=z.
    pub fn letter() -> DFA {
        DFA {
            alphabet_segmentation:alphabet::Segmentation::from_divisions(&[97,123]),
            links:Matrix::from(vec![vec![INVALID,1,INVALID], vec![INVALID,INVALID,INVALID]]),
            callbacks:vec![
                None,
                Some(RuleExecutable {priority:2, code:"group_0_rule_0".into()}),
            ],
        }
    }

    /// DFA automata that accepts any number of spaces ' '.
    pub fn spaces() -> DFA {
        DFA {
            alphabet_segmentation:alphabet::Segmentation::from_divisions(&[0,32,33]),
            links:Matrix::from(vec![
                vec![INVALID,1,INVALID],
                vec![INVALID,2,INVALID],
                vec![INVALID,2,INVALID],
            ]),
            callbacks:vec![
                None,
                Some(RuleExecutable {priority:3, code:"group_0_rule_0".into()}),
                Some(RuleExecutable {priority:3, code:"group_0_rule_0".into()}),
            ],
        }
    }

    /// DFA automata that accepts one letter a..=z or any many spaces.
    pub fn letter_and_spaces() -> DFA {
        DFA {
            alphabet_segmentation:alphabet::Segmentation::from_divisions(&[32,33,97,123]),
            links:Matrix::from(vec![
                vec![INVALID,      1,INVALID,      2,INVALID],
                vec![INVALID,      3,INVALID,INVALID,INVALID],
                vec![INVALID,INVALID,INVALID,INVALID,INVALID],
                vec![INVALID,      3,INVALID,INVALID,INVALID],
            ]),
            callbacks:vec![
                None,
                Some(RuleExecutable {priority:4, code:"group_0_rule_1".into()}),
                Some(RuleExecutable {priority:4, code:"group_0_rule_0".into()}),
                Some(RuleExecutable {priority:4, code:"group_0_rule_1".into()}),
            ],
        }
    }

    // #[bench]
    // fn bench_to_dfa_newline(bencher:&mut Bencher) {
    //     bencher.iter(|| DFA::from(&newline()))
    // }
    //
    // #[bench]
    // fn bench_to_dfa_letter(bencher:&mut Bencher) {
    //     bencher.iter(|| DFA::from(&letter()))
    // }
    //
    // #[bench]
    // fn bench_to_dfa_spaces(bencher:&mut Bencher) {
    //     bencher.iter(|| DFA::from(&spaces()))
    // }
    //
    // #[bench]
    // fn bench_to_dfa_letter_and_spaces(bencher:&mut Bencher) {
    //     bencher.iter(|| DFA::from(&letter_and_spaces()))
    // }
}
