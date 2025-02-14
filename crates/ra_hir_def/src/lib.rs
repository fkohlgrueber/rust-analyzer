//! `hir_def` crate contains everything between macro expansion and type
//! inference.
//!
//! It defines various items (structs, enums, traits) which comprises Rust code,
//! as well as an algorithm for resolving paths to such entities.
//!
//! Note that `hir_def` is a work in progress, so not all of the above is
//! actually true.

pub mod db;
pub mod attr;
pub mod path;
pub mod type_ref;
pub mod builtin_type;
pub mod adt;
pub mod impls;
pub mod diagnostics;
pub mod expr;
pub mod body;
pub mod generics;
pub mod traits;

#[cfg(test)]
mod test_db;
#[cfg(test)]
mod marks;

// FIXME: this should be private
pub mod nameres;

use std::hash::{Hash, Hasher};

use hir_expand::{ast_id_map::FileAstId, db::AstDatabase, AstId, HirFileId, Source};
use ra_arena::{impl_arena_id, RawId};
use ra_db::{salsa, CrateId, FileId};
use ra_syntax::{ast, AstNode, SyntaxNode};

use crate::{builtin_type::BuiltinType, db::InternDatabase};

pub enum ModuleSource {
    SourceFile(ast::SourceFile),
    Module(ast::Module),
}

impl ModuleSource {
    pub fn new(
        db: &impl db::DefDatabase2,
        file_id: Option<FileId>,
        decl_id: Option<AstId<ast::Module>>,
    ) -> ModuleSource {
        match (file_id, decl_id) {
            (Some(file_id), _) => {
                let source_file = db.parse(file_id).tree();
                ModuleSource::SourceFile(source_file)
            }
            (None, Some(item_id)) => {
                let module = item_id.to_node(db);
                assert!(module.item_list().is_some(), "expected inline module");
                ModuleSource::Module(module)
            }
            (None, None) => panic!(),
        }
    }

    // FIXME: this methods do not belong here
    pub fn from_position(
        db: &impl db::DefDatabase2,
        position: ra_db::FilePosition,
    ) -> ModuleSource {
        let parse = db.parse(position.file_id);
        match &ra_syntax::algo::find_node_at_offset::<ast::Module>(
            parse.tree().syntax(),
            position.offset,
        ) {
            Some(m) if !m.has_semi() => ModuleSource::Module(m.clone()),
            _ => {
                let source_file = parse.tree();
                ModuleSource::SourceFile(source_file)
            }
        }
    }

    pub fn from_child_node(db: &impl db::DefDatabase2, child: Source<&SyntaxNode>) -> ModuleSource {
        if let Some(m) =
            child.value.ancestors().filter_map(ast::Module::cast).find(|it| !it.has_semi())
        {
            ModuleSource::Module(m)
        } else {
            let file_id = child.file_id.original_file(db);
            let source_file = db.parse(file_id).tree();
            ModuleSource::SourceFile(source_file)
        }
    }

    pub fn from_file_id(db: &impl db::DefDatabase2, file_id: FileId) -> ModuleSource {
        let source_file = db.parse(file_id).tree();
        ModuleSource::SourceFile(source_file)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId {
    pub krate: CrateId,
    pub module_id: CrateModuleId,
}

/// An ID of a module, **local** to a specific crate
// FIXME: rename to `LocalModuleId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrateModuleId(RawId);
impl_arena_id!(CrateModuleId);

macro_rules! impl_intern_key {
    ($name:ident) => {
        impl salsa::InternKey for $name {
            fn from_intern_id(v: salsa::InternId) -> Self {
                $name(v)
            }
            fn as_intern_id(&self) -> salsa::InternId {
                self.0
            }
        }
    };
}

#[derive(Debug)]
pub struct ItemLoc<N: AstNode> {
    pub(crate) module: ModuleId,
    ast_id: AstId<N>,
}

impl<N: AstNode> PartialEq for ItemLoc<N> {
    fn eq(&self, other: &Self) -> bool {
        self.module == other.module && self.ast_id == other.ast_id
    }
}
impl<N: AstNode> Eq for ItemLoc<N> {}
impl<N: AstNode> Hash for ItemLoc<N> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.module.hash(hasher);
        self.ast_id.hash(hasher);
    }
}

impl<N: AstNode> Clone for ItemLoc<N> {
    fn clone(&self) -> ItemLoc<N> {
        ItemLoc { module: self.module, ast_id: self.ast_id }
    }
}

#[derive(Clone, Copy)]
pub struct LocationCtx<DB> {
    db: DB,
    module: ModuleId,
    file_id: HirFileId,
}

impl<'a, DB> LocationCtx<&'a DB> {
    pub fn new(db: &'a DB, module: ModuleId, file_id: HirFileId) -> LocationCtx<&'a DB> {
        LocationCtx { db, module, file_id }
    }
}

impl<'a, DB: AstDatabase + InternDatabase> LocationCtx<&'a DB> {
    pub fn to_def<N, DEF>(self, ast: &N) -> DEF
    where
        N: AstNode,
        DEF: AstItemDef<N>,
    {
        DEF::from_ast(self, ast)
    }
}

pub trait AstItemDef<N: AstNode>: salsa::InternKey + Clone {
    fn intern(db: &impl InternDatabase, loc: ItemLoc<N>) -> Self;
    fn lookup_intern(self, db: &impl InternDatabase) -> ItemLoc<N>;

    fn from_ast(ctx: LocationCtx<&(impl AstDatabase + InternDatabase)>, ast: &N) -> Self {
        let items = ctx.db.ast_id_map(ctx.file_id);
        let item_id = items.ast_id(ast);
        Self::from_ast_id(ctx, item_id)
    }
    fn from_ast_id(ctx: LocationCtx<&impl InternDatabase>, ast_id: FileAstId<N>) -> Self {
        let loc = ItemLoc { module: ctx.module, ast_id: AstId::new(ctx.file_id, ast_id) };
        Self::intern(ctx.db, loc)
    }
    fn source(self, db: &(impl AstDatabase + InternDatabase)) -> Source<N> {
        let loc = self.lookup_intern(db);
        let value = loc.ast_id.to_node(db);
        Source { file_id: loc.ast_id.file_id(), value }
    }
    fn module(self, db: &impl InternDatabase) -> ModuleId {
        let loc = self.lookup_intern(db);
        loc.module
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(salsa::InternId);
impl_intern_key!(FunctionId);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionLoc {
    pub container: ContainerId,
    pub ast_id: AstId<ast::FnDef>,
}

impl Intern for FunctionLoc {
    type ID = FunctionId;
    fn intern(self, db: &impl db::DefDatabase2) -> FunctionId {
        db.intern_function(self)
    }
}

impl Lookup for FunctionId {
    type Data = FunctionLoc;
    fn lookup(&self, db: &impl db::DefDatabase2) -> FunctionLoc {
        db.lookup_intern_function(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructOrUnionId(salsa::InternId);
impl_intern_key!(StructOrUnionId);
impl AstItemDef<ast::StructDef> for StructOrUnionId {
    fn intern(db: &impl InternDatabase, loc: ItemLoc<ast::StructDef>) -> Self {
        db.intern_struct_or_union(loc)
    }
    fn lookup_intern(self, db: &impl InternDatabase) -> ItemLoc<ast::StructDef> {
        db.lookup_intern_struct_or_union(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId(pub StructOrUnionId);
impl From<StructId> for StructOrUnionId {
    fn from(id: StructId) -> StructOrUnionId {
        id.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnionId(pub StructOrUnionId);
impl From<UnionId> for StructOrUnionId {
    fn from(id: UnionId) -> StructOrUnionId {
        id.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumId(salsa::InternId);
impl_intern_key!(EnumId);
impl AstItemDef<ast::EnumDef> for EnumId {
    fn intern(db: &impl InternDatabase, loc: ItemLoc<ast::EnumDef>) -> Self {
        db.intern_enum(loc)
    }
    fn lookup_intern(self, db: &impl InternDatabase) -> ItemLoc<ast::EnumDef> {
        db.lookup_intern_enum(self)
    }
}

// FIXME: rename to `VariantId`, only enums can ave variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantId {
    pub parent: EnumId,
    pub local_id: LocalEnumVariantId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalEnumVariantId(RawId);
impl_arena_id!(LocalEnumVariantId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VariantId {
    EnumVariantId(EnumVariantId),
    StructId(StructId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructFieldId {
    parent: VariantId,
    local_id: LocalStructFieldId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalStructFieldId(RawId);
impl_arena_id!(LocalStructFieldId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstId(salsa::InternId);
impl_intern_key!(ConstId);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstLoc {
    pub container: ContainerId,
    pub ast_id: AstId<ast::ConstDef>,
}

impl Intern for ConstLoc {
    type ID = ConstId;
    fn intern(self, db: &impl db::DefDatabase2) -> ConstId {
        db.intern_const(self)
    }
}

impl Lookup for ConstId {
    type Data = ConstLoc;
    fn lookup(&self, db: &impl db::DefDatabase2) -> ConstLoc {
        db.lookup_intern_const(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticId(salsa::InternId);
impl_intern_key!(StaticId);
impl AstItemDef<ast::StaticDef> for StaticId {
    fn intern(db: &impl InternDatabase, loc: ItemLoc<ast::StaticDef>) -> Self {
        db.intern_static(loc)
    }
    fn lookup_intern(self, db: &impl InternDatabase) -> ItemLoc<ast::StaticDef> {
        db.lookup_intern_static(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraitId(salsa::InternId);
impl_intern_key!(TraitId);
impl AstItemDef<ast::TraitDef> for TraitId {
    fn intern(db: &impl InternDatabase, loc: ItemLoc<ast::TraitDef>) -> Self {
        db.intern_trait(loc)
    }
    fn lookup_intern(self, db: &impl InternDatabase) -> ItemLoc<ast::TraitDef> {
        db.lookup_intern_trait(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeAliasId(salsa::InternId);
impl_intern_key!(TypeAliasId);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeAliasLoc {
    pub container: ContainerId,
    pub ast_id: AstId<ast::TypeAliasDef>,
}

impl Intern for TypeAliasLoc {
    type ID = TypeAliasId;
    fn intern(self, db: &impl db::DefDatabase2) -> TypeAliasId {
        db.intern_type_alias(self)
    }
}

impl Lookup for TypeAliasId {
    type Data = TypeAliasLoc;
    fn lookup(&self, db: &impl db::DefDatabase2) -> TypeAliasLoc {
        db.lookup_intern_type_alias(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplId(salsa::InternId);
impl_intern_key!(ImplId);
impl AstItemDef<ast::ImplBlock> for ImplId {
    fn intern(db: &impl InternDatabase, loc: ItemLoc<ast::ImplBlock>) -> Self {
        db.intern_impl(loc)
    }
    fn lookup_intern(self, db: &impl InternDatabase) -> ItemLoc<ast::ImplBlock> {
        db.lookup_intern_impl(self)
    }
}

macro_rules! impl_froms {
    ($e:ident: $($v:ident $(($($sv:ident),*))?),*) => {
        $(
            impl From<$v> for $e {
                fn from(it: $v) -> $e {
                    $e::$v(it)
                }
            }
            $($(
                impl From<$sv> for $e {
                    fn from(it: $sv) -> $e {
                        $e::$v($v::$sv(it))
                    }
                }
            )*)?
        )*
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerId {
    ModuleId(ModuleId),
    ImplId(ImplId),
    TraitId(TraitId),
}

/// A Data Type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AdtId {
    StructId(StructId),
    UnionId(UnionId),
    EnumId(EnumId),
}
impl_froms!(AdtId: StructId, UnionId, EnumId);

/// The defs which can be visible in the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleDefId {
    ModuleId(ModuleId),
    FunctionId(FunctionId),
    AdtId(AdtId),
    // Can't be directly declared, but can be imported.
    EnumVariantId(EnumVariantId),
    ConstId(ConstId),
    StaticId(StaticId),
    TraitId(TraitId),
    TypeAliasId(TypeAliasId),
    BuiltinType(BuiltinType),
}
impl_froms!(
    ModuleDefId: ModuleId,
    FunctionId,
    AdtId(StructId, EnumId, UnionId),
    EnumVariantId,
    ConstId,
    StaticId,
    TraitId,
    TypeAliasId,
    BuiltinType
);

/// The defs which have a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefWithBodyId {
    FunctionId(FunctionId),
    StaticId(StaticId),
    ConstId(ConstId),
}

impl_froms!(DefWithBodyId: FunctionId, ConstId, StaticId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AssocItemId {
    FunctionId(FunctionId),
    ConstId(ConstId),
    TypeAliasId(TypeAliasId),
}
// FIXME: not every function, ... is actually an assoc item. maybe we should make
// sure that you can only turn actual assoc items into AssocItemIds. This would
// require not implementing From, and instead having some checked way of
// casting them, and somehow making the constructors private, which would be annoying.
impl_froms!(AssocItemId: FunctionId, ConstId, TypeAliasId);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GenericDefId {
    FunctionId(FunctionId),
    AdtId(AdtId),
    TraitId(TraitId),
    TypeAliasId(TypeAliasId),
    ImplId(ImplId),
    // enum variants cannot have generics themselves, but their parent enums
    // can, and this makes some code easier to write
    EnumVariantId(EnumVariantId),
    // consts can have type parameters from their parents (i.e. associated consts of traits)
    ConstId(ConstId),
}
impl_froms!(
    GenericDefId: FunctionId,
    AdtId(StructId, EnumId, UnionId),
    TraitId,
    TypeAliasId,
    ImplId,
    EnumVariantId,
    ConstId
);

trait Intern {
    type ID;
    fn intern(self, db: &impl db::DefDatabase2) -> Self::ID;
}

pub trait Lookup {
    type Data;
    fn lookup(&self, db: &impl db::DefDatabase2) -> Self::Data;
}

pub trait HasModule {
    fn module(&self, db: &impl db::DefDatabase2) -> ModuleId;
}

impl HasModule for FunctionLoc {
    fn module(&self, db: &impl db::DefDatabase2) -> ModuleId {
        match self.container {
            ContainerId::ModuleId(it) => it,
            ContainerId::ImplId(it) => it.module(db),
            ContainerId::TraitId(it) => it.module(db),
        }
    }
}

impl HasModule for TypeAliasLoc {
    fn module(&self, db: &impl db::DefDatabase2) -> ModuleId {
        match self.container {
            ContainerId::ModuleId(it) => it,
            ContainerId::ImplId(it) => it.module(db),
            ContainerId::TraitId(it) => it.module(db),
        }
    }
}

impl HasModule for ConstLoc {
    fn module(&self, db: &impl db::DefDatabase2) -> ModuleId {
        match self.container {
            ContainerId::ModuleId(it) => it,
            ContainerId::ImplId(it) => it.module(db),
            ContainerId::TraitId(it) => it.module(db),
        }
    }
}

pub trait HasSource {
    type Value;
    fn source(&self, db: &impl db::DefDatabase2) -> Source<Self::Value>;
}

impl HasSource for FunctionLoc {
    type Value = ast::FnDef;

    fn source(&self, db: &impl db::DefDatabase2) -> Source<ast::FnDef> {
        let node = self.ast_id.to_node(db);
        Source::new(self.ast_id.file_id(), node)
    }
}

impl HasSource for TypeAliasLoc {
    type Value = ast::TypeAliasDef;

    fn source(&self, db: &impl db::DefDatabase2) -> Source<ast::TypeAliasDef> {
        let node = self.ast_id.to_node(db);
        Source::new(self.ast_id.file_id(), node)
    }
}

impl HasSource for ConstLoc {
    type Value = ast::ConstDef;

    fn source(&self, db: &impl db::DefDatabase2) -> Source<ast::ConstDef> {
        let node = self.ast_id.to_node(db);
        Source::new(self.ast_id.file_id(), node)
    }
}
