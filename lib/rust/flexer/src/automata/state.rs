//! This module exports State implementation for Nondeterministic Finite Automata.

use crate::automata::alphabet;
use crate::automata::symbol::Symbol;

use crate::prelude::*;



// ===========
// == State ==
// ===========

/// A named state for a [`super::nfa::NFA`].
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub struct State {
    /// A set of transitions that can trigger without consuming a symbol (ε-transitions).
    epsilon_links:Vec<Identifier>,
    /// The set of transitions that trigger while consuming a specific symbol.
    ///
    /// When triggered, the automaton will transition to the [`Transition::target_state`].
    links:Vec<Transition>,
    /// The name of the state.
    ///
    /// This is used to auto-generate a call to the rust method of the same name.
    name:Option<String>,
    /// The function to call when evaluating the state.
    callback:String
}

impl State {
    /// Updater for field `name`. Returns updated state.
    pub fn named(mut self, name:&str) -> Self {
        self.name = Some(name.to_owned());
        self
    }

    /// Add `link` to the current state.
    pub fn add_link(&mut self, link:Transition) {
        self.links.push(link);
    }

    /// Add an epsilon link transitioning to `identifier` to the current state.
    pub fn add_epsilon_link(&mut self, identifier:Identifier) {
        self.epsilon_links.push(identifier);
    }

    /// Set the name for this state to `name`.
    pub fn set_name(&mut self, name:Option<String>) {
        self.name = name.into();
    }

    /// Set the callback for this state to `callback`.
    pub fn set_callback(&mut self, callback:impl Str) {
        self.callback = callback.into();
    }

    /// Get a reference to the links in this state.
    pub fn links(&self) -> &Vec<Transition> {
        &self.links
    }

    /// Get a reference to the epsilon links in this state.
    pub fn epsilon_links(&self) -> &Vec<Identifier> {
        &self.epsilon_links
    }

    /// Get a reference to the name for this state.
    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    /// Get a reference to the callback for this state.
    pub fn callback(&self) -> &String {
        &self.callback
    }

    /// Returns transition (next state) for each symbol in the alphabet.
    pub fn targets(&self, alphabet:&alphabet::Segmentation) -> Vec<Identifier> {
        let mut targets = vec![];
        let mut index   = 0;
        let mut links   = self.links.clone();
        links.sort_by_key(|link| *link.symbols.start());
        for &symbol in alphabet.divisions() {
            while links.len() > index && *links[index].symbols.end() < symbol {
                index += 1;
            }
            if links.len() <= index || *links[index].symbols.start() > symbol {
                targets.push(Identifier::INVALID);
            } else {
                targets.push(links[index].target_state);
            }
        }
        targets
    }
}


// === Trait Impls ====

impl From<Vec<usize>> for State {
    /// Creates a state with epsilon links.
    fn from(vec:Vec<usize>) -> Self {
        let epsilon_links = vec.iter().cloned().map(|id| Identifier{id}).collect();
        State{epsilon_links,..Default::default()}
    }
}

impl From<Vec<Link>> for State {
    /// Creates a state with ordinary links.
    fn from(vec:Vec<Link>) -> Self {
        let links = vec.iter().cloned().map(|link| {
            let range = link.range;
            let start = Symbol::from(*range.start());
            let end   = Symbol::from(*range.end());
            Transition::new(start..=end,Identifier::new(link.target_id))
        }).collect();
        State{links,..Default::default()}
    }
}



// ============
// === Link ===
// ============

/// A representation of a link within a state.
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct Link {
    /// The range on which the link triggers.
    pub range : RangeInclusive<u32>,
    /// The target identifier for the link.
    pub target_id : usize
}

impl Link {
    /// Construct a new link with the specified `range` and `id`.
    pub fn new(range:RangeInclusive<u32>, target_id:usize) -> Self {
        Self{range,target_id}
    }
}



// ================
// == Identifier ==
// ================

/// A state identifier for an arbitrary finite automaton.
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
#[allow(missing_docs)]
pub struct Identifier {
    pub id: usize
}

impl Identifier {
    /// An identifier representing the invalid state.
    ///
    /// When in an invalid state, a finite automaton will reject the sequence of input symbols.
    pub const INVALID:Identifier = Identifier{id:usize::max_value()};

    /// Constructs a new state identifier.
    pub fn new(id:usize) -> Identifier {
        Identifier{id}
    }
}

// === Trait Impls ===

impl Default for Identifier {
    /// Returns state::INVALID. This is because every finite automata has an invalid state
    /// and because all transitions in automata transition matrix lead to invalid state by default.
    fn default() -> Self {
        Identifier::INVALID
    }
}

impl From<usize> for Identifier {
    fn from(id: usize) -> Self {
        Identifier{id}
    }
}



// ============
// === Link ===
// ============

/// A transition between states in a finite automaton that must consume a symbol to trigger.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Transition {
    /// The range of symbols on which this transition will trigger.
    pub symbols:RangeInclusive<Symbol>,
    /// The state that is entered after the transition has triggered.
    pub target_state:Identifier,
}

impl Transition {
    /// Construct a new transition.
    pub fn new(symbols:RangeInclusive<Symbol>, target_state:Identifier) -> Self {
        Self{symbols,target_state}
    }
}



// =============
// === Tests ===
// =============

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::alphabet;

    #[test]
    fn default_identifier_invalid() {
        assert_eq!(Identifier::default(),Identifier::INVALID);
    }

    #[test]
    fn state_default() {
        let state = State::default();
        assert!(state.epsilon_links().is_empty());
        assert!(state.links().is_empty());
        assert!(state.name().is_none());
        assert_eq!(state.callback(),&("".to_string()));
    }

    #[test]
    fn state_named() {
        let state = State::default();
        let named = state.named("Foo");
        assert_eq!(named.name(),&Some("Foo".to_string()));
    }

    #[test]
    fn state_targets() {
        let alphabet = alphabet::Segmentation::from_divisions(&[0,5,10,15,25,50]);
        let state = State::from(vec![
            Link::new(0..=10,1),
            Link::new(5..=15,2),
        ]);
        let targets = state.targets(&alphabet);
        let expected_targets:Vec<Identifier> = vec![
            Identifier::new(1),
            Identifier::new(1),
            Identifier::new(1),
            Identifier::new(2),
            Identifier::INVALID,
            Identifier::INVALID,
        ];
        assert_eq!(expected_targets,targets);
    }
}
