//! Utility module for converting between hir_def ids and code_model wrappers.
//!
//! It's unclear if we need this long-term, but it's definitelly useful while we
//! are splitting the hir.

use hir_def::{AdtId, AssocItemId, DefWithBodyId, EnumVariantId, GenericDefId, ModuleDefId};

use crate::{Adt, AssocItem, DefWithBody, EnumVariant, GenericDef, ModuleDef};

macro_rules! from_id {
    ($(($id:path, $ty:path)),*) => {$(
        impl From<$id> for $ty {
            fn from(id: $id) -> $ty {
                $ty { id }
            }
        }
    )*}
}

from_id![
    (hir_def::ModuleId, crate::Module),
    (hir_def::StructId, crate::Struct),
    (hir_def::UnionId, crate::Union),
    (hir_def::EnumId, crate::Enum),
    (hir_def::TypeAliasId, crate::TypeAlias),
    (hir_def::TraitId, crate::Trait),
    (hir_def::StaticId, crate::Static),
    (hir_def::ConstId, crate::Const),
    (hir_def::FunctionId, crate::Function),
    (hir_def::ImplId, crate::ImplBlock),
    (hir_expand::MacroDefId, crate::MacroDef)
];

impl From<AdtId> for Adt {
    fn from(id: AdtId) -> Self {
        match id {
            AdtId::StructId(it) => Adt::Struct(it.into()),
            AdtId::UnionId(it) => Adt::Union(it.into()),
            AdtId::EnumId(it) => Adt::Enum(it.into()),
        }
    }
}

impl From<Adt> for AdtId {
    fn from(id: Adt) -> Self {
        match id {
            Adt::Struct(it) => AdtId::StructId(it.id),
            Adt::Union(it) => AdtId::UnionId(it.id),
            Adt::Enum(it) => AdtId::EnumId(it.id),
        }
    }
}

impl From<EnumVariantId> for EnumVariant {
    fn from(id: EnumVariantId) -> Self {
        EnumVariant { parent: id.parent.into(), id: id.local_id }
    }
}

impl From<ModuleDefId> for ModuleDef {
    fn from(id: ModuleDefId) -> Self {
        match id {
            ModuleDefId::ModuleId(it) => ModuleDef::Module(it.into()),
            ModuleDefId::FunctionId(it) => ModuleDef::Function(it.into()),
            ModuleDefId::AdtId(it) => ModuleDef::Adt(it.into()),
            ModuleDefId::EnumVariantId(it) => ModuleDef::EnumVariant(it.into()),
            ModuleDefId::ConstId(it) => ModuleDef::Const(it.into()),
            ModuleDefId::StaticId(it) => ModuleDef::Static(it.into()),
            ModuleDefId::TraitId(it) => ModuleDef::Trait(it.into()),
            ModuleDefId::TypeAliasId(it) => ModuleDef::TypeAlias(it.into()),
            ModuleDefId::BuiltinType(it) => ModuleDef::BuiltinType(it),
        }
    }
}

impl From<DefWithBody> for DefWithBodyId {
    fn from(def: DefWithBody) -> Self {
        match def {
            DefWithBody::Function(it) => DefWithBodyId::FunctionId(it.id),
            DefWithBody::Static(it) => DefWithBodyId::StaticId(it.id),
            DefWithBody::Const(it) => DefWithBodyId::ConstId(it.id),
        }
    }
}

impl From<AssocItemId> for AssocItem {
    fn from(def: AssocItemId) -> Self {
        match def {
            AssocItemId::FunctionId(it) => AssocItem::Function(it.into()),
            AssocItemId::TypeAliasId(it) => AssocItem::TypeAlias(it.into()),
            AssocItemId::ConstId(it) => AssocItem::Const(it.into()),
        }
    }
}

impl From<GenericDef> for GenericDefId {
    fn from(def: GenericDef) -> Self {
        match def {
            GenericDef::Function(it) => GenericDefId::FunctionId(it.id),
            GenericDef::Adt(it) => GenericDefId::AdtId(it.into()),
            GenericDef::Trait(it) => GenericDefId::TraitId(it.id),
            GenericDef::TypeAlias(it) => GenericDefId::TypeAliasId(it.id),
            GenericDef::ImplBlock(it) => GenericDefId::ImplId(it.id),
            GenericDef::EnumVariant(it) => {
                GenericDefId::EnumVariantId(EnumVariantId { parent: it.parent.id, local_id: it.id })
            }
            GenericDef::Const(it) => GenericDefId::ConstId(it.id),
        }
    }
}

impl From<GenericDefId> for GenericDef {
    fn from(def: GenericDefId) -> Self {
        match def {
            GenericDefId::FunctionId(it) => GenericDef::Function(it.into()),
            GenericDefId::AdtId(it) => GenericDef::Adt(it.into()),
            GenericDefId::TraitId(it) => GenericDef::Trait(it.into()),
            GenericDefId::TypeAliasId(it) => GenericDef::TypeAlias(it.into()),
            GenericDefId::ImplId(it) => GenericDef::ImplBlock(it.into()),
            GenericDefId::EnumVariantId(it) => GenericDef::EnumVariant(it.into()),
            GenericDefId::ConstId(it) => GenericDef::Const(it.into()),
        }
    }
}
