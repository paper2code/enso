//! This module exports jni bindings generator.

use std::collections::HashMap  as Map;
use std::collections::BTreeSet as Set;
use super::Generator;
use super::ast::*;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::quote;
use crate::generation::types;
use itertools::Itertools;



// =============================
// === Module Representation ===
// =============================

/// A representation of rust module.
#[derive(Debug,Clone,Default)]
pub struct Collector {
    /// Package name.
    pub package: String,
    /// Currently active module.
    pub module: Vec<Name>,
    /// Set of all type names.
    pub types: Set<Name>,
    /// Vector of all available classes and their arguments.
    pub classes: Vec<(Class,Vec<Type>)>,
    /// Map from variants to types (i.e. Some => Option).
    pub extends: Map<Name,Name>,
    /// Set of generic parameters a type is used with.
    pub generics: Map<Name,Set<Type>>,
}

impl Collector {
    /// Adds a class to collector.
    pub fn add_class(&mut self, mut class:Class) {
        let args = class.args.iter().map(|(name, typ)|
            (name.clone(), self.monomorphize(&typ).1)
        ).collect();
        let arg_types  = class.args.into_iter().map(|(_,typ)| typ);
        class.typ.path = self.module.clone();
        self.classes.push((Class{typ:class.typ, args}, arg_types.collect()));
    }

    /// Returns a monomorphized name and the original type with monorphized arguments.
    pub fn monomorphize(&mut self, typ:&Type) -> (Name, Type) {
        let mut uid  = typ.name.str.clone();
        let mut args = vec![];
        let     path = vec![];
        for arg in typ.args.iter() {
            let (name, typ) = self.monomorphize(&arg);
            uid.push_str(&name.str);
            args.push(typ);
        }
        if super::types::builtin(&typ.name).is_some() {
            return (Name(uid), Type{name:typ.name.clone(), path, args});
        }
        let alias = Type{name:Name(&uid), path, args};
        self.generics.entry(typ.name.clone()).or_insert(Set::new()).insert(alias);
        return (Name(&uid), Type::from(Name(&uid)));
    }
}


// === Generator Impls ===

impl Generator<Collector> for &File {
    fn write(self, source:&mut Collector) {
        source.package = self.package.clone();
        Module {name:self.name.clone(), lines:&self.content.items[..]}.write(source);
    }
}

impl<'a> Generator<Collector> for &Module<'a> {
    fn write(self, source:&mut Collector) {
        source.module.push(self.name.clone());
        for item in self.lines {
            match item {
                syn::Item::Mod   (val) => Module::from(val).write(source),
                syn::Item::Type  (val) => TypeAlias::from(val).write(source),
                syn::Item::Struct(val) => Class::from(val).write(source),
                syn::Item::Enum  (val) => Enum::from(val).write(source),
                _                      => (),
            }
        }
        source.module.pop();
    }
}

impl Generator<Collector> for &TypeAlias {
    fn write(self, source:&mut Collector) {
        if self.val.args.is_empty() {
            source.monomorphize(&self.val);
        }
    }
}

impl Generator<Collector> for &Class {
    fn write(self, source:&mut Collector) {
        source.add_class(self.clone());
        source.types.insert(self.typ.name.clone());
        if self.typ.args.is_empty() {
            source.monomorphize(&self.typ);
        }
    }
}

impl Generator<Collector> for &Enum {
    fn write(self, source:&mut Collector) {
        for (_, variant) in &self.variants {
            source.extends.insert(variant.typ.name.clone(), self.typ.name.clone());
            source.add_class(variant.clone());
        }
        source.types.insert(self.typ.name.clone());
        if self.typ.args.is_empty() {
            source.monomorphize(&self.typ);
        }
    }
}


// === ToTokens Impls ===

impl ToTokens for Name {
    fn to_tokens(&self, tokens:&mut TokenStream) {
        syn::Ident::new(&self.str, proc_macro2::Span::call_site()).to_tokens(tokens)
    }
}



// ======================
// === Rust Generator ===
// ======================

/// An associated types in trait with monorphized signature.
#[derive(Debug,Clone)]
pub struct AssociatedType {
    /// Name of the type associated type.
    pub name: Name,
    /// The class the associated type represents.
    pub class: Class
}

impl AssociatedType {
    /// Ast tree of any type used in trait.
    ///
    /// For custom types this returns `<Self as Api>::Name`.
    /// For builtin types this returns `Name<typ(arg1), typ(arg2)..>`.
    pub fn typ(typ:&Type) -> TokenStream {
        let args = typ.args.iter().map(Self::typ);
        let name = &typ.name;
        if types::builtin(&name).is_none() {
            quote!(<Self as Api>::#name)
        } else {
            quote!(#name<#(#args),*>)
        }
    }

    /// An api of function that constructs the given associated type.
    ///
    /// For example `fn name(x:i64, y:<Self as Api>::Y) -> <Self as Api>::Name`
    pub fn fun(&self) -> TokenStream {
        let typ = &self.class.typ.name;
        let fun = &name::var(&typ);
        let arg = self.class.args.iter().map(|(ref name, ref typ)| {
            let typ = Self::typ(typ);
            quote!(#name:#typ)
        });

        quote!(fn #fun(&self, #(#arg),*) -> <Self as Api>::#typ)
    }

}

/// A generator of an API for AST construction.
#[derive(Debug,Clone)]
pub struct Source {
    /// Set of all class names.
    pub class_names: Set<Name>,
    /// Vector of all classes.
    pub classes: Vec<Class>,
    /// Name of the scala package.
    pub package: String,
    /// Vector of user defined associated types.
    pub types: Vec<AssociatedType>
}

impl Source {
    /// Generates the AST trait.
    pub fn ast_trait(&self) -> TokenStream {
        let types   = self.types.iter().map(|obj| &obj.class.typ.name);
        let funs    = self.types.iter().map(|obj| obj.fun());

        quote! {
            trait Api {
                #(type #types);*;

                #(#funs);*;
            }
        }
    }

    /// Generates the Rust struct that is used to construct Rust AST.
    pub fn rust_struct(&self) -> TokenStream {
        quote!{
            #[derive(Debug,Clone,Copy,Default)]
            pub struct Rust;
        }
    }

    /// Generates an implementation of AST trait for Rust AST.
    pub fn rust_impl(&self) -> TokenStream {
        let types    = self.types.iter().map(|obj| {
            let name = &obj.class.typ.name;
            let typ  = &obj.name;
            let args = obj.class.typ.args.iter().map(AssociatedType::typ);
            quote!(#name = #typ<#(#args),*>)
        });
        let funs     = self.types.iter().map(|obj| {
            let fun  = obj.fun();
            let typ  = &obj.name;
            let args = obj.class.args.iter().map(|(name, _)| name);
            quote!(#fun { #typ{#(#args),*} })
        });

        quote! {
            impl Api for Rust {
                #(type #types);*;

                #(#funs)*
            }
        }
    }

    /// Generates the Scala struct that is used to construct the Scala AST.
    pub fn scala_struct(&self) -> TokenStream {
        let fields  = self.classes.iter().map(|obj| name::var(&obj.typ.name)).collect_vec();
        let objects = self.classes.iter().map(|obj| {
            let mut name = String::from("");
            let mut args = String::from("(");
            types::jni_name(&mut name, self.package.as_str(), &obj.typ);
            for (_, typ) in &obj.args {
                if let Some(name) = types::builtin(&typ.name) {
                    args += name.jni;
                } else if !self.class_names.contains(&typ.name) {
                    args += "Ljava/lang/Object;";
                } else {
                    types::jni_name(&mut args, self.package.as_str(), &typ);
                }
            }
            args += ")V";
            quote!(Object::new(&env,#name,#args))
        });


        quote! {
            use crate::generation::types::Object;
            use crate::generation::types::StdLib;
            use jni::JNIEnv;

            #[derive(Clone)]
            pub struct Scala<'a> {
                pub env:&'a JNIEnv<'a>,
                pub lib:StdLib<'a>,
                #(pub #fields:Object<'a>),*
            }

            impl<'a> Scala<'a> {
                pub fn new(env:&'a JNIEnv<'a>) -> Self {
                    Self { env, lib:StdLib::new(env), #(#fields:#objects),* }
                }
            }
        }
    }

    /// Generates the implementation of AST trait for the Scala AST.
    pub fn scala_impl(&self) -> TokenStream {
        let types    = self.types.iter().map(|obj| &obj.class.typ.name);
        let funs     = self.types.iter().map(|obj| {
            let fun  = obj.fun();
            let typ  = &name::var(&obj.name);
            let args = obj.class.args.iter().map(|(name,_)| name);
            quote!(#fun { self.#typ.init(&[#(#args.into()),*]) })
        });

        quote! {
            use jni::objects::JObject;

            impl<'a> Api for Scala<'a> {
                #(type #types = JObject<'a>);*;

                #(#funs)*
            }
        }
    }

    /// Generates the AST trait and implementation for Scala and Rust AST.
    pub fn ast_api(&self) -> TokenStream {
        let rust_struct  = self.rust_struct();
        let scala_struct = self.scala_struct();
        let ast_trait    = self.ast_trait();
        let rust_impl    = self.rust_impl();
        let scala_impl   = self.scala_impl();

        quote! {
            #rust_struct
            #scala_struct

            #ast_trait
            #rust_impl
            #scala_impl
        }
    }
}


// === From Impls ===

impl From<File> for Collector {
    fn from(file:File) -> Self {
        let mut result = Collector::default();
        file.write(&mut result);
        result
    }
}

impl From<Collector> for Source {
    fn from(mut collector:Collector) -> Self {
        fn apply(typ:&Type, vars:&Map<&Name,&Type>) -> Type {
            if let Some(&typ) = vars.get(&typ.name) { typ.clone() } else {
                let args = typ.args.iter().map(|typ| apply(typ,&vars)).collect();
                Type{name:typ.name.clone(), path:vec![], args}
            }
        }

        let mut classes = vec![];
        let mut types   = vec![];
        for (class, args) in collector.classes {
            let name = collector.extends.get(&class.typ.name).unwrap_or_else(||&class.typ.name).clone();
            for typ in collector.generics.remove(&name).unwrap_or_default() {
                let vars = class.typ.args.iter().map(|t| &t.name).zip(&typ.args).collect();
                let args = class.args.iter().map(|(name,typ)|
                    (name.clone(), apply(&typ, &vars))
                ).collect();
                types.push(AssociatedType{name:name.clone(), class:Class{typ, args}})
            }
            let args = args.into_iter().map(|typ| (Name(""),typ)).collect();
            classes.push(Class{typ:class.typ, args})
        }
        Self {class_names:collector.types, classes, package:collector.package, types}
    }
}


// ========================
// === Name Conversions ===
// ========================

/// Module for name manipulation.
pub mod name {
    use crate::generation::ast::Name;
    use inflector::Inflector;



    /// Creates a Rust type name `foo_bar => FooBar`.
    pub fn typ(name:&Name) -> Name {
        let mut string = name.str.to_camel_case();
        string[0..1].make_ascii_uppercase();
        string.into()
    }

    /// Creates a Rust variable name `FooBar => foo_bar`.
    pub fn var(name:&Name) -> Name {
        let mut name = name.str.to_snake_case();
        name[0..1].make_ascii_lowercase();
        name.into()
    }
}



// =============
// === Tests ===
// =============

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote as parse;



    #[test]
    fn test_classes() {
        let rust = File::new("", "", parse! {
            pub enum FooBarBaz {
                Foo(a::Foo),
                Bar(a::Bar),
                Baz(b::Baz),
            }
            mod a {
                struct Foo {x:Box<Vec<i32>>}
                struct Bar {x:usize, y:u8  }
            }
        });
        let classes = vec![
            Class::from(&parse!(struct Foo {variant:Foo})),
            Class::from(&parse!(struct Bar {variant:Bar})),
            Class::from(&parse!(struct Baz {variant:Baz})),
            Class::from(&parse!(struct Foo {x:Box<Vec<i32>>})),
            Class::from(&parse!(struct Bar {x:usize, y:u8  })),
        ];
        assert_eq!(Collector::from(rust).classes.into_iter().map(|(x,_)|x).collect::<Vec<_>>(), classes);
    }


    #[test]
    fn test_monomorphization() {
        let     generics = Collector::from(File::new("","",parse!(struct A(B<X,Y>, C<B<X,Box<i32>>>);))).generics;
        let     boxi32   = Type{name:Name("Box"), path:vec![], args:vec![Name("i32").into()]};
        let mut expected = Map::new();
        let mut set      = Set::new();

        set.insert(Type{name:Name("BXY"), path:vec![], args:vec![Name("X").into(), Name("Y").into()]});
        set.insert(Type{name:Name("BXBoxi32"), path:vec![], args:vec![Name("X").into(), boxi32]});
        expected.insert(Name("B"), std::mem::replace(&mut set, Set::new()));

        set.insert(Type{name:Name("CBXBoxi32"), path:vec![], args:vec![Name("BXBoxi32").into()]});
        expected.insert(Name("C"), std::mem::replace(&mut set, Set::new()));

        set.insert(Name("A").into());
        expected.insert(Name("A"), std::mem::replace(&mut set, Set::new()));

        set.insert(Name("X").into());
        expected.insert(Name("X"), std::mem::replace(&mut set, Set::new()));

        set.insert(Name("Y").into());
        expected.insert(Name("Y"), std::mem::replace(&mut set, Set::new()));

        assert_eq!(generics, expected);
    }

    #[test]
    fn test_api() {
        let source = Source::from(Collector::from(File::new("", "", parse!{
            struct A(B<X,Y>, C<B<X,Box<i32>>>);
            struct B<X,Y>(X,Y);
            struct C<T>(T);
        })));
        let expected = quote! {
            trait Api {
                type A;
                type BXBoxi32;
                type BXY;
                type CBXBoxi32;

                fn a
                (val0:<Self as Api>::BXY, val1:<Self as Api>::CBXBoxi32) -> <Self as Api>::A;
                fn bx_boxi_32
                (val0:<Self as Api>::X, val1:Box<i32 <> >) -> <Self as Api>::BXBoxi32;
                fn bxy
                (val0:<Self as Api>::X, val1:<Self as Api>::Y) -> <Self as Api>::BXY;
                fn cbx_boxi_32
                (val0:<Self as Api>::BXBoxi32) -> <Self as Api>::CBXBoxi32;
            }
        };
        assert_eq!(source.ast_trait().to_string(), expected.to_string())
    }

    #[test]
    fn test_rust() {
        let source = Source::from(Collector::from(File::new("", "", parse!{
            struct A(B<X,Y>, C<B<X,Box<i32>>>);
            struct B<X,Y>(X,Y);
            struct C<T>(T);
        })));
        let expected = quote! {
            impl Api for Rust {
                type A         = A<>;
                type BXBoxi32  = B< <Self as Api>::X, Box<i32 <> > >;
                type BXY       = B< <Self as Api>::X, <Self as Api>::Y>;
                type CBXBoxi32 = C< <Self as Api>::BXBoxi32>;
                fn a
                (val0:<Self as Api>::BXY, val1:<Self as Api>::CBXBoxi32) -> <Self as Api>::A { A{val0, val1} }
                fn bx_boxi_32
                (val0:<Self as Api>::X, val1:Box<i32 <> >) -> <Self as Api>::BXBoxi32 { B{val0, val1} }
                fn bxy
                (val0:<Self as Api>::X, val1:<Self as Api>::Y) -> <Self as Api>::BXY { B{val0, val1} }
                fn cbx_boxi_32
                (val0:<Self as Api>::BXBoxi32) -> <Self as Api>::CBXBoxi32 { C{val0} }
            }
        };
        assert_eq!(source.rust_impl().to_string(), expected.to_string())
    }

    #[test]
    fn test_scalaa() {
        let source = Source::from(Collector::from(File::new("Ast", "ast", parse!{
            struct A{b:B<i8,u8>, c:C<B<i32,Vec<i64>>>}
            struct B<X,Y>{x:X,y:Y}
            struct C<T>{t:T}
        })));
        let expected = quote! {
            use crate::generation::types::Object;
            use crate::generation::types::StdLib;
            use jni::JNIEnv;
            use jni::objects::JObject;

            pub struct Scala<'a> { pub env: &'a JNIEnv<'a>, pub lib: StdLib<'a>, pub a: Object<'a>, pub b: Object<'a>, pub c: Object<'a> }

            impl<'a> Scala<'a> {
                pub fn new(env: &'a JNIEnv<'a>) -> Self {
                    Self {
                        env,
                        lib: StdLib::new(env),
                        a: Object::new(&env, "Last$Ast$A;", "(Last$B;Last$C;)V"),
                        b: Object::new(&env, "Last$Ast$B;", "(Ljava/lang/Object;Ljava/lang/Object;)V"),
                        c: Object::new(&env, "Last$Ast$C;", "(Ljava/lang/Object;)V"),
                    }
                }
            }

            impl<'a> Api for Scala<'a> {
                type A = JObject<'a>;
                type BcharBoxi32 = JObject<'a>;
                type Bi8u8 = JObject<'a>;
                type CBcharBoxi32 = JObject<'a>;
                fn a(&self, val0: <Self as Api>::Bi8u8, val1: <Self as Api>::CBcharBoxi32) -> <Self as Api>::A { self.a.init(&[val0.into(), val1.into()]) }
                fn bchar_boxi_32(&self, val0: char<>, val1: Box<i32<>>) -> <Self as Api>::BcharBoxi32 { self.b.init(&[val0.into(), val1.into()]) }
                fn bi_8u_8(&self, val0: i8<>, val1: u8<>) -> <Self as Api>::Bi8u8 { self.b.init(&[val0.into(), val1.into()]) }
                fn c_bchar_boxi_32(&self, val0: <Self as Api>::BcharBoxi32) -> <Self as Api>::CBcharBoxi32 { self.c.init(&[val0.into()]) }
            }
        };

        assert_eq!(source.ast_api().to_string(), expected.to_string())
    }
}