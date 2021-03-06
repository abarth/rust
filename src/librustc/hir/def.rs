// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use hir::def_id::DefId;
use util::nodemap::{NodeMap, DefIdMap};
use syntax::ast;
use syntax::ext::base::MacroKind;
use syntax_pos::Span;
use hir;
use ty;

#[derive(Clone, Copy, PartialEq, Eq, RustcEncodable, RustcDecodable, Hash, Debug)]
pub enum CtorKind {
    /// Constructor function automatically created by a tuple struct/variant.
    Fn,
    /// Constructor constant automatically created by a unit struct/variant.
    Const,
    /// Unusable name in value namespace created by a struct variant.
    Fictive,
}

#[derive(Clone, Copy, PartialEq, Eq, RustcEncodable, RustcDecodable, Hash, Debug)]
pub enum Def {
    // Type namespace
    Mod(DefId),
    Struct(DefId), // DefId refers to NodeId of the struct itself
    Union(DefId),
    Enum(DefId),
    Variant(DefId),
    Trait(DefId),
    TyAlias(DefId),
    TyForeign(DefId),
    TraitAlias(DefId),
    AssociatedTy(DefId),
    PrimTy(hir::PrimTy),
    TyParam(DefId),
    SelfTy(Option<DefId> /* trait */, Option<DefId> /* impl */),

    // Value namespace
    Fn(DefId),
    Const(DefId),
    Static(DefId, bool /* is_mutbl */),
    StructCtor(DefId, CtorKind), // DefId refers to NodeId of the struct's constructor
    VariantCtor(DefId, CtorKind),
    Method(DefId),
    AssociatedConst(DefId),

    Local(ast::NodeId),
    Upvar(ast::NodeId,  // node id of closed over local
          usize,        // index in the freevars list of the closure
          ast::NodeId), // expr node that creates the closure
    Label(ast::NodeId),

    // Macro namespace
    Macro(DefId, MacroKind),

    GlobalAsm(DefId),

    // Both namespaces
    Err,
}

/// The result of resolving a path before lowering to HIR.
/// `base_def` is definition of resolved part of the
/// path, `unresolved_segments` is the number of unresolved
/// segments.
///
/// ```text
/// module::Type::AssocX::AssocY::MethodOrAssocType
/// ^~~~~~~~~~~~  ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
/// base_def      unresolved_segments = 3
///
/// <T as Trait>::AssocX::AssocY::MethodOrAssocType
///       ^~~~~~~~~~~~~~  ^~~~~~~~~~~~~~~~~~~~~~~~~
///       base_def        unresolved_segments = 2
/// ```
#[derive(Copy, Clone, Debug)]
pub struct PathResolution {
    base_def: Def,
    unresolved_segments: usize,
}

impl PathResolution {
    pub fn new(def: Def) -> Self {
        PathResolution { base_def: def, unresolved_segments: 0 }
    }

    pub fn with_unresolved_segments(def: Def, mut unresolved_segments: usize) -> Self {
        if def == Def::Err { unresolved_segments = 0 }
        PathResolution { base_def: def, unresolved_segments: unresolved_segments }
    }

    #[inline]
    pub fn base_def(&self) -> Def {
        self.base_def
    }

    #[inline]
    pub fn unresolved_segments(&self) -> usize {
        self.unresolved_segments
    }

    pub fn kind_name(&self) -> &'static str {
        if self.unresolved_segments != 0 {
            "associated item"
        } else {
            self.base_def.kind_name()
        }
    }
}

/// Definition mapping
pub type DefMap = NodeMap<PathResolution>;

/// This is the replacement export map. It maps a module to all of the exports
/// within.
pub type ExportMap = DefIdMap<Vec<Export>>;

#[derive(Copy, Clone, Debug, RustcEncodable, RustcDecodable)]
pub struct Export {
    /// The name of the target.
    pub ident: ast::Ident,
    /// The definition of the target.
    pub def: Def,
    /// The span of the target definition.
    pub span: Span,
    /// The visibility of the export.
    /// We include non-`pub` exports for hygienic macros that get used from extern crates.
    pub vis: ty::Visibility,
    /// True if from a `use` or and `extern crate`.
    /// Used in rustdoc.
    pub is_import: bool,
}

impl CtorKind {
    pub fn from_ast(vdata: &ast::VariantData) -> CtorKind {
        match *vdata {
            ast::VariantData::Tuple(..) => CtorKind::Fn,
            ast::VariantData::Unit(..) => CtorKind::Const,
            ast::VariantData::Struct(..) => CtorKind::Fictive,
        }
    }
    pub fn from_hir(vdata: &hir::VariantData) -> CtorKind {
        match *vdata {
            hir::VariantData::Tuple(..) => CtorKind::Fn,
            hir::VariantData::Unit(..) => CtorKind::Const,
            hir::VariantData::Struct(..) => CtorKind::Fictive,
        }
    }
}

impl Def {
    pub fn def_id(&self) -> DefId {
        match *self {
            Def::Fn(id) | Def::Mod(id) | Def::Static(id, _) |
            Def::Variant(id) | Def::VariantCtor(id, ..) | Def::Enum(id) |
            Def::TyAlias(id) | Def::TraitAlias(id) |
            Def::AssociatedTy(id) | Def::TyParam(id) | Def::Struct(id) | Def::StructCtor(id, ..) |
            Def::Union(id) | Def::Trait(id) | Def::Method(id) | Def::Const(id) |
            Def::AssociatedConst(id) | Def::Macro(id, ..) |
            Def::GlobalAsm(id) | Def::TyForeign(id) => {
                id
            }

            Def::Local(..) |
            Def::Upvar(..) |
            Def::Label(..)  |
            Def::PrimTy(..) |
            Def::SelfTy(..) |
            Def::Err => {
                bug!("attempted .def_id() on invalid def: {:?}", self)
            }
        }
    }

    /// A human readable kind name
    pub fn kind_name(&self) -> &'static str {
        match *self {
            Def::Fn(..) => "function",
            Def::Mod(..) => "module",
            Def::Static(..) => "static",
            Def::Variant(..) => "variant",
            Def::VariantCtor(.., CtorKind::Fn) => "tuple variant",
            Def::VariantCtor(.., CtorKind::Const) => "unit variant",
            Def::VariantCtor(.., CtorKind::Fictive) => "struct variant",
            Def::Enum(..) => "enum",
            Def::TyAlias(..) => "type alias",
            Def::TraitAlias(..) => "trait alias",
            Def::AssociatedTy(..) => "associated type",
            Def::Struct(..) => "struct",
            Def::StructCtor(.., CtorKind::Fn) => "tuple struct",
            Def::StructCtor(.., CtorKind::Const) => "unit struct",
            Def::StructCtor(.., CtorKind::Fictive) => bug!("impossible struct constructor"),
            Def::Union(..) => "union",
            Def::Trait(..) => "trait",
            Def::TyForeign(..) => "foreign type",
            Def::Method(..) => "method",
            Def::Const(..) => "constant",
            Def::AssociatedConst(..) => "associated constant",
            Def::TyParam(..) => "type parameter",
            Def::PrimTy(..) => "builtin type",
            Def::Local(..) => "local variable",
            Def::Upvar(..) => "closure capture",
            Def::Label(..) => "label",
            Def::SelfTy(..) => "self type",
            Def::Macro(.., macro_kind) => macro_kind.descr(),
            Def::GlobalAsm(..) => "global asm",
            Def::Err => "unresolved item",
        }
    }
}
