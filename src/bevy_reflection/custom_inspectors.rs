/*
 * Bevy Debugger MCP Server - Custom Inspectors
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Custom Inspectors for Complex Bevy Types
//!
//! This module provides specialized inspectors for complex Bevy component types
//! that require domain-specific knowledge for proper inspection and visualization.

use serde_json::{json, Value};
use crate::bevy_reflection::inspector::{CustomInspector, InspectedValue};
use crate::brp_messages::ComponentValue;
use crate::error::{Error, Result};

/// Inspector for Option<T> types - handles Some/None variants
pub struct OptionInspector;

impl CustomInspector for OptionInspector {
    fn inspect(&self, value: &ComponentValue, type_name: &str) -> Result<InspectedValue> {
        match value {
            Value::Null => {
                Ok(InspectedValue {
                    name: "Option".to_string(),
                    raw_value: value.clone(),
                    type_info: type_name.to_string(),
                    display_value: "None".to_string(),
                    inspectable: false,
                    children: None,
                })
            }
            Value::Object(obj) if obj.contains_key("Some") => {
                let some_value = &obj["Some"];
                Ok(InspectedValue {
                    name: "Option".to_string(),
                    raw_value: value.clone(),
                    type_info: type_name.to_string(),
                    display_value: format!("Some({})", self.format_value_preview(some_value)),
                    inspectable: true,
                    children: Some(vec![InspectedValue {
                        name: "Some".to_string(),
                        raw_value: some_value.clone(),
                        type_info: self.infer_inner_type(some_value),
                        display_value: self.format_value_preview(some_value),
                        inspectable: self.is_value_inspectable(some_value),
                        children: None,
                    }]),
                })
            }
            _ => {
                // Assume non-null value is Some variant
                Ok(InspectedValue {
                    name: "Option".to_string(),
                    raw_value: value.clone(),
                    type_info: type_name.to_string(),
                    display_value: format!("Some({})", self.format_value_preview(value)),
                    inspectable: true,
                    children: Some(vec![InspectedValue {
                        name: "Some".to_string(),
                        raw_value: value.clone(),
                        type_info: self.infer_inner_type(value),
                        display_value: self.format_value_preview(value),
                        inspectable: self.is_value_inspectable(value),
                        children: None,
                    }]),
                })
            }
        }
    }

    fn name(&self) -> &str {
        "OptionInspector"
    }

    fn supported_types(&self) -> Vec<String> {
        vec![
            "core::option::Option".to_string(),
            "Option".to_string(),
        ]
    }
}

impl OptionInspector {
    fn format_value_preview(&self, value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => format!("\"{}\"", s),
            Value::Array(arr) => format!("[{} items]", arr.len()),
            Value::Object(obj) => format!("{{ {} fields }}", obj.len()),
        }
    }

    fn infer_inner_type(&self, value: &Value) -> String {
        match value {
            Value::Null => "()".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Number(n) => {
                if n.is_i64() { "i64".to_string() }
                else if n.is_u64() { "u64".to_string() }
                else { "f64".to_string() }
            }
            Value::String(_) => "String".to_string(),
            Value::Array(_) => "Vec<T>".to_string(),
            Value::Object(_) => "Struct".to_string(),
        }
    }

    fn is_value_inspectable(&self, value: &Value) -> bool {
        matches!(value, Value::Object(_) | Value::Array(_))
    }
}

/// Inspector for Vec<T> and other list types
pub struct VecInspector;

impl CustomInspector for VecInspector {
    fn inspect(&self, value: &ComponentValue, type_name: &str) -> Result<InspectedValue> {
        if let Value::Array(arr) = value {
            let mut children = Vec::new();
            
            // Inspect each element
            for (index, item) in arr.iter().enumerate() {
                children.push(InspectedValue {
                    name: format!("[{}]", index),
                    raw_value: item.clone(),
                    type_info: self.infer_element_type(item),
                    display_value: self.format_element(item),
                    inspectable: self.is_value_inspectable(item),
                    children: None,
                });
            }

            // Show only first few elements for large arrays
            let display_children = if children.len() > 10 {
                let mut truncated = children.into_iter().take(10).collect::<Vec<_>>();
                truncated.push(InspectedValue {
                    name: format!("... and {} more", arr.len() - 10),
                    raw_value: Value::Null,
                    type_info: "...".to_string(),
                    display_value: format!("({} total elements)", arr.len()),
                    inspectable: false,
                    children: None,
                });
                truncated
            } else {
                children
            };

            Ok(InspectedValue {
                name: "Vec".to_string(),
                raw_value: value.clone(),
                type_info: type_name.to_string(),
                display_value: format!("Vec<T>[{}]", arr.len()),
                inspectable: !arr.is_empty(),
                children: if arr.is_empty() { None } else { Some(display_children) },
            })
        } else {
            Err(Error::DebugError("Vec inspector expects array value".to_string()))
        }
    }

    fn name(&self) -> &str {
        "VecInspector"
    }

    fn supported_types(&self) -> Vec<String> {
        vec![
            "alloc::vec::Vec".to_string(),
            "Vec".to_string(),
            "std::vec::Vec".to_string(),
        ]
    }
}

impl VecInspector {
    fn infer_element_type(&self, value: &Value) -> String {
        match value {
            Value::Null => "()".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Number(n) => {
                if n.is_i64() { "i64".to_string() }
                else if n.is_u64() { "u64".to_string() }
                else { "f64".to_string() }
            }
            Value::String(_) => "String".to_string(),
            Value::Array(_) => "Vec<T>".to_string(),
            Value::Object(_) => "T".to_string(),
        }
    }

    fn format_element(&self, value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => format!("\"{}\"", if s.len() > 30 { format!("{}...", &s[..30]) } else { s.clone() }),
            Value::Array(arr) => format!("[{} items]", arr.len()),
            Value::Object(obj) => format!("{{ {} }}", obj.keys().take(3).cloned().collect::<Vec<_>>().join(", ")),
        }
    }

    fn is_value_inspectable(&self, value: &Value) -> bool {
        matches!(value, Value::Object(_) | Value::Array(_))
    }
}

/// Inspector for HashMap<K, V> and other map types
pub struct HashMapInspector;

impl CustomInspector for HashMapInspector {
    fn inspect(&self, value: &ComponentValue, type_name: &str) -> Result<InspectedValue> {
        if let Value::Object(obj) = value {
            let mut children = Vec::new();
            
            // Inspect each key-value pair
            for (key, val) in obj {
                children.push(InspectedValue {
                    name: format!("[\"{}\"]", key),
                    raw_value: json!({"key": key, "value": val}),
                    type_info: format!("({}, {})", self.infer_key_type(key), self.infer_value_type(val)),
                    display_value: format!("{}: {}", key, self.format_value(val)),
                    inspectable: self.is_value_inspectable(val),
                    children: None,
                });
            }

            // Show only first few entries for large maps
            let display_children = if children.len() > 15 {
                let mut truncated = children.into_iter().take(15).collect::<Vec<_>>();
                truncated.push(InspectedValue {
                    name: format!("... and {} more", obj.len() - 15),
                    raw_value: Value::Null,
                    type_info: "...".to_string(),
                    display_value: format!("({} total entries)", obj.len()),
                    inspectable: false,
                    children: None,
                });
                truncated
            } else {
                children
            };

            Ok(InspectedValue {
                name: "HashMap".to_string(),
                raw_value: value.clone(),
                type_info: type_name.to_string(),
                display_value: format!("HashMap<K,V>[{}]", obj.len()),
                inspectable: !obj.is_empty(),
                children: if obj.is_empty() { None } else { Some(display_children) },
            })
        } else {
            Err(Error::DebugError("HashMap inspector expects object value".to_string()))
        }
    }

    fn name(&self) -> &str {
        "HashMapInspector"
    }

    fn supported_types(&self) -> Vec<String> {
        vec![
            "std::collections::HashMap".to_string(),
            "HashMap".to_string(),
            "std::collections::hash_map::HashMap".to_string(),
        ]
    }
}

impl HashMapInspector {
    fn infer_key_type(&self, _key: &str) -> String {
        "String".to_string() // JSON keys are always strings
    }

    fn infer_value_type(&self, value: &Value) -> String {
        match value {
            Value::Null => "()".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Number(n) => {
                if n.is_i64() { "i64".to_string() }
                else if n.is_u64() { "u64".to_string() }
                else { "f64".to_string() }
            }
            Value::String(_) => "String".to_string(),
            Value::Array(_) => "Vec<T>".to_string(),
            Value::Object(_) => "V".to_string(),
        }
    }

    fn format_value(&self, value: &Value) -> String {
        match value {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => format!("\"{}\"", if s.len() > 20 { format!("{}...", &s[..20]) } else { s.clone() }),
            Value::Array(arr) => format!("[{} items]", arr.len()),
            Value::Object(obj) => format!("{{ {} fields }}", obj.len()),
        }
    }

    fn is_value_inspectable(&self, value: &Value) -> bool {
        matches!(value, Value::Object(_) | Value::Array(_))
    }
}

/// Inspector for Bevy Entity references
pub struct EntityInspector;

impl CustomInspector for EntityInspector {
    fn inspect(&self, value: &ComponentValue, type_name: &str) -> Result<InspectedValue> {
        let entity_id = match value {
            Value::Number(n) => n.as_u64().unwrap_or(0),
            Value::Object(obj) => {
                // Look for common entity ID fields
                obj.get("id")
                    .or_else(|| obj.get("entity"))
                    .or_else(|| obj.get("index"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            }
            _ => 0,
        };

        let generation = if let Value::Object(obj) = value {
            obj.get("generation").and_then(|v| v.as_u64()).unwrap_or(0)
        } else {
            0
        };

        let display = if generation > 0 {
            format!("Entity(id: {}, gen: {})", entity_id, generation)
        } else {
            format!("Entity({})", entity_id)
        };

        Ok(InspectedValue {
            name: "Entity".to_string(),
            raw_value: value.clone(),
            type_info: type_name.to_string(),
            display_value: display,
            inspectable: false, // Entities themselves aren't inspectable via reflection
            children: None,
        })
    }

    fn name(&self) -> &str {
        "EntityInspector"
    }

    fn supported_types(&self) -> Vec<String> {
        vec![
            "bevy_ecs::entity::Entity".to_string(),
            "Entity".to_string(),
        ]
    }
}

/// Inspector for Bevy Color types
pub struct ColorInspector;

impl CustomInspector for ColorInspector {
    fn inspect(&self, value: &ComponentValue, type_name: &str) -> Result<InspectedValue> {
        let (color_info, children) = match value {
            Value::Object(obj) => {
                let mut kids = Vec::new();
                let mut display_parts = Vec::new();

                // Check for different color formats
                if let (Some(r), Some(g), Some(b)) = (
                    obj.get("r").and_then(|v| v.as_f64()),
                    obj.get("g").and_then(|v| v.as_f64()),
                    obj.get("b").and_then(|v| v.as_f64()),
                ) {
                    let a = obj.get("a").and_then(|v| v.as_f64()).unwrap_or(1.0);
                    
                    kids.push(InspectedValue {
                        name: "r".to_string(),
                        raw_value: json!(r),
                        type_info: "f32".to_string(),
                        display_value: format!("{:.3}", r),
                        inspectable: false,
                        children: None,
                    });
                    
                    kids.push(InspectedValue {
                        name: "g".to_string(),
                        raw_value: json!(g),
                        type_info: "f32".to_string(),
                        display_value: format!("{:.3}", g),
                        inspectable: false,
                        children: None,
                    });
                    
                    kids.push(InspectedValue {
                        name: "b".to_string(),
                        raw_value: json!(b),
                        type_info: "f32".to_string(),
                        display_value: format!("{:.3}", b),
                        inspectable: false,
                        children: None,
                    });
                    
                    if a != 1.0 {
                        kids.push(InspectedValue {
                            name: "a".to_string(),
                            raw_value: json!(a),
                            type_info: "f32".to_string(),
                            display_value: format!("{:.3}", a),
                            inspectable: false,
                            children: None,
                        });
                    }

                    let hex = self.rgba_to_hex(r, g, b, a);
                    display_parts.push(format!("RGBA({:.2}, {:.2}, {:.2}, {:.2})", r, g, b, a));
                    display_parts.push(format!("#{}", hex));
                }

                let display = if display_parts.is_empty() {
                    "Color { unknown format }".to_string()
                } else {
                    format!("Color {{ {} }}", display_parts.join(" | "))
                };

                (display, Some(kids))
            }
            _ => ("Invalid Color".to_string(), None),
        };

        Ok(InspectedValue {
            name: "Color".to_string(),
            raw_value: value.clone(),
            type_info: type_name.to_string(),
            display_value: color_info,
            inspectable: children.is_some(),
            children,
        })
    }

    fn name(&self) -> &str {
        "ColorInspector"
    }

    fn supported_types(&self) -> Vec<String> {
        vec![
            "bevy_render::color::Color".to_string(),
            "bevy_color::color::Color".to_string(),
            "Color".to_string(),
        ]
    }
}

impl ColorInspector {
    fn rgba_to_hex(&self, r: f64, g: f64, b: f64, a: f64) -> String {
        let r_int = ((r * 255.0).round() as u8).clamp(0, 255);
        let g_int = ((g * 255.0).round() as u8).clamp(0, 255);
        let b_int = ((b * 255.0).round() as u8).clamp(0, 255);
        
        if a >= 0.99 {
            format!("{:02X}{:02X}{:02X}", r_int, g_int, b_int)
        } else {
            let a_int = ((a * 255.0).round() as u8).clamp(0, 255);
            format!("{:02X}{:02X}{:02X}{:02X}", r_int, g_int, b_int, a_int)
        }
    }
}

/// Factory for creating all default custom inspectors
pub fn create_default_inspectors() -> Vec<Box<dyn CustomInspector + Send + Sync>> {
    vec![
        Box::new(OptionInspector),
        Box::new(VecInspector),
        Box::new(HashMapInspector),
        Box::new(EntityInspector),
        Box::new(ColorInspector),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_inspector_none() {
        let inspector = OptionInspector;
        let value = Value::Null;
        let result = inspector.inspect(&value, "Option<String>").unwrap();
        
        assert_eq!(result.display_value, "None");
        assert!(!result.inspectable);
    }

    #[test]
    fn test_option_inspector_some() {
        let inspector = OptionInspector;
        let value = json!({"Some": "hello"});
        let result = inspector.inspect(&value, "Option<String>").unwrap();
        
        assert!(result.display_value.contains("Some"));
        assert!(result.inspectable);
        assert!(result.children.is_some());
    }

    #[test]
    fn test_vec_inspector() {
        let inspector = VecInspector;
        let value = json!([1, 2, 3, 4, 5]);
        let result = inspector.inspect(&value, "Vec<i32>").unwrap();
        
        assert_eq!(result.display_value, "Vec<T>[5]");
        assert!(result.inspectable);
        assert_eq!(result.children.as_ref().unwrap().len(), 5);
    }

    #[test]
    fn test_hashmap_inspector() {
        let inspector = HashMapInspector;
        let value = json!({"key1": "value1", "key2": "value2"});
        let result = inspector.inspect(&value, "HashMap<String, String>").unwrap();
        
        assert_eq!(result.display_value, "HashMap<K,V>[2]");
        assert!(result.inspectable);
        assert_eq!(result.children.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_entity_inspector() {
        let inspector = EntityInspector;
        let value = json!({"id": 123, "generation": 1});
        let result = inspector.inspect(&value, "Entity").unwrap();
        
        assert!(result.display_value.contains("Entity"));
        assert!(result.display_value.contains("123"));
        assert!(result.display_value.contains("gen: 1"));
    }

    #[test]
    fn test_color_inspector() {
        let inspector = ColorInspector;
        let value = json!({"r": 1.0, "g": 0.5, "b": 0.0, "a": 1.0});
        let result = inspector.inspect(&value, "Color").unwrap();
        
        assert!(result.display_value.contains("RGBA"));
        assert!(result.display_value.contains("#"));
        assert!(result.inspectable);
        assert_eq!(result.children.as_ref().unwrap().len(), 3); // r, g, b (a=1.0 is omitted)
    }

    #[test]
    fn test_color_to_hex() {
        let inspector = ColorInspector;
        
        // Pure red
        assert_eq!(inspector.rgba_to_hex(1.0, 0.0, 0.0, 1.0), "FF0000");
        
        // Semi-transparent blue
        assert_eq!(inspector.rgba_to_hex(0.0, 0.0, 1.0, 0.5), "0000FF80");
    }

    #[test]
    fn test_large_vec_truncation() {
        let inspector = VecInspector;
        let large_vec: Vec<i32> = (0..20).collect();
        let value = json!(large_vec);
        let result = inspector.inspect(&value, "Vec<i32>").unwrap();
        
        // Should show 10 items + "... and X more"
        assert_eq!(result.children.as_ref().unwrap().len(), 11);
        assert!(result.children.as_ref().unwrap().last().unwrap().name.contains("and"));
    }
}