/*
 * Bevy Debugger MCP Server - Bevy Reflection Module
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

//! Bevy Reflection Integration Module
//! 
//! This module contains the complete reflection integration system including:
//! - Core reflection inspector and metadata structures
//! - Custom inspectors for Bevy-specific types
//! - TypeRegistry integration tools
//! - Reflection-based query optimization

pub mod inspector;
pub mod custom_inspectors;
pub mod type_registry_tools;
pub mod reflection_queries;

// Re-export main types from inspector module
pub use inspector::{
    BevyReflectionInspector, ReflectionMetadata, FieldMetadata, TypeCategory,
    ReflectionInspectionResult, InspectedValue, CustomInspector,
    ReflectionDiffResult, FieldDiff, ChangeType, ChangeSeverity, DiffSummary,
    TransformInspector,
};

// Export submodule types
pub use custom_inspectors::*;
pub use type_registry_tools::*;
pub use reflection_queries::*;