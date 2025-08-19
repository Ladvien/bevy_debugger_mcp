#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}

struct HighlightMaterial {
    color: vec4<f32>,
    mode: u32,
    thickness: f32,
    intensity: f32,
    time: f32,
}

@group(2) @binding(0) var<uniform> material: HighlightMaterial;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let model = mesh_functions::get_model_matrix(vertex.instance_index);
    out.world_position = model * vec4<f32>(vertex.position, 1.0);
    out.world_normal = normalize((model * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.clip_position = position_world_to_clip(out.world_position.xyz);
    out.uv = vertex.uv;
    
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var base_color = material.color;
    
    // Apply different highlight modes
    if material.mode == 0u {
        // Outline mode - create outline effect
        let edge_factor = 1.0 - abs(dot(normalize(in.world_normal), normalize(in.world_position.xyz)));
        let outline_strength = pow(edge_factor, 1.0 / material.thickness);
        base_color = vec4<f32>(base_color.rgb, base_color.a * outline_strength);
        
    } else if material.mode == 1u {
        // Tint mode - blend with existing color
        base_color = vec4<f32>(base_color.rgb, base_color.a * 0.5);
        
    } else if material.mode == 2u {
        // Glow mode - animated glow effect
        let pulse = (sin(material.time * 2.0) + 1.0) * 0.5;
        let glow_intensity = material.intensity * (0.5 + pulse * 0.5);
        base_color = vec4<f32>(base_color.rgb * glow_intensity, base_color.a);
        
    } else if material.mode == 3u {
        // Wireframe mode - show edges only
        let wire_factor = abs(fract(in.uv.x * 20.0) - 0.5) + abs(fract(in.uv.y * 20.0) - 0.5);
        if wire_factor > material.thickness {
            discard;
        }
        
    } else {
        // Solid color mode - replace with highlight color
        // base_color remains as is
    }
    
    return base_color;
}