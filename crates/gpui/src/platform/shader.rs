use std::fmt::Display;

use smallvec::SmallVec;

use crate::SharedString;

#[derive(Clone, PartialEq, Eq, Hash)]
#[expect(missing_docs)]
pub struct CustomShaderInfo {
    pub main_body: SharedString,
    pub extra_items: SmallVec<[SharedString; 4]>,
    pub data_name: &'static str,
    pub data_definition: Option<&'static str>,
    pub data_size: usize,
    pub data_align: usize,
    pub backdrop_read: bool,
}

impl Display for CustomShaderInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let instance_data_definition = self.data_definition.unwrap_or("");
        let main_body = &self.main_body;
        let extra_items = self.extra_items.join("");

        let backdrop_code = if self.backdrop_read {
            "
            @group(1) @binding(1)
            var t_backdrop: texture_2d<f32>;
            @group(1) @binding(2)
            var s_backdrop: sampler;

            fn sample_backdrop(position: vec2<f32>, scale_factor: f32) -> vec4<f32> {
                let uv = position * scale_factor / globals.viewport_size;
                return textureSample(t_backdrop, s_backdrop, uv);
            }
            "
        } else {
            ""
        };
        let (instance_data_field, instance_data_param, instance_data_arg) = if self.data_size != 0 {
            (
                format!("instance_data: {}", self.data_name),
                format!(", data: {}", self.data_name),
                ", b_instances.instances[input.instance_id].instance_data",
            )
        } else {
            (String::new(), String::new(), "")
        };

        write!(
            f,
            r#"
        struct GlobalParams {{
            viewport_size: vec2<f32>,
            premultiplied_alpha: u32,
            pad: u32,
        }}

        @group(0) @binding(0) var<uniform> globals: GlobalParams;
        {backdrop_code}

        fn to_device_position(unit_vertex: vec2<f32>, bounds: Bounds) -> vec2<f32> {{
            let position = unit_vertex * bounds.size + bounds.origin;
            return position / globals.viewport_size * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
        }}

        fn distance_from_clip_rect(unit_vertex: vec2<f32>, bounds: Bounds, clip_bounds: Bounds) -> vec4<f32> {{
            let position = unit_vertex * bounds.size + bounds.origin;
            let tl = position - clip_bounds.origin;
            let br = clip_bounds.origin + clip_bounds.size - position;
            return vec4<f32>(tl.x, br.x, tl.y, br.y);
        }}

        struct Bounds {{
            origin: vec2<f32>,
            size: vec2<f32>,
        }}

        {extra_items}
        {instance_data_definition}

        struct Instance {{
            bounds: Bounds,
            content_mask: Bounds,
            opacity: f32,
            scale_factor: f32,
            {instance_data_field}
        }}

        struct Instances {{
            instances: array<Instance>,
        }}

        @group(1) @binding(0) var<storage, read> b_instances: Instances;

        struct VertexOut {{
            @builtin(position) position: vec4<f32>,
            @location(0) clip_distances: vec4<f32>,
            @location(1) origin: vec2<f32>,
            @location(2) size: vec2<f32>,
            @location(3) opacity: f32,
            @location(4) scale_factor: f32,
            @location(5) @interpolate(flat) instance_id: u32,
        }}

        @vertex
        fn vs(@builtin(vertex_index) vertex_id: u32, @builtin(instance_index) instance_id: u32) -> VertexOut {{
            let unit_vertex = vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
            let instance = b_instances.instances[instance_id];

            var out = VertexOut();
            out.position = vec4<f32>(to_device_position(unit_vertex, instance.bounds), 0.0, 1.0);
            out.clip_distances = distance_from_clip_rect(unit_vertex, instance.bounds, instance.content_mask);
            out.origin = instance.bounds.origin / instance.scale_factor;
            out.size = instance.bounds.size / instance.scale_factor;
            out.opacity = instance.opacity;
            out.scale_factor = instance.scale_factor;
            out.instance_id = instance_id;

            return out;
        }}

        fn user_fs(position: vec2<f32>, bounds: Bounds, scale_factor: f32{instance_data_param}) -> vec4<f32> {{
            {main_body}
        }}

        @fragment
        fn fs(input: VertexOut) -> @location(0) vec4<f32> {{
            if (any(input.clip_distances < vec4<f32>(0.0))) {{
                return vec4<f32>(0.0);
            }}

            let color = user_fs(
                input.position.xy / input.scale_factor,
                Bounds(input.origin, input.size),
                input.scale_factor
                {instance_data_arg}
            );

            let alpha = color.a * input.opacity;
            let multiplier = select(1.0, alpha, globals.premultiplied_alpha != 0u);
            return vec4<f32>(color.rgb * multiplier, alpha);
        }}
        "#
        )
    }
}

#[cfg(test)]
mod tests {
    use smallvec::SmallVec;

    use super::CustomShaderInfo;

    fn shader_info(backdrop_read: bool) -> CustomShaderInfo {
        CustomShaderInfo {
            main_body: "return vec4<f32>(1.0);".into(),
            extra_items: SmallVec::new(),
            data_name: "f32",
            data_definition: None,
            data_size: 0,
            data_align: 4,
            backdrop_read,
        }
    }

    #[test]
    fn emits_backdrop_bindings_for_backdrop_shaders() {
        let source = shader_info(true).to_string();

        assert!(source.contains("@group(1) @binding(1)"));
        assert!(source.contains("var t_backdrop: texture_2d<f32>;"));
        assert!(source.contains("@group(1) @binding(2)"));
        assert!(source.contains("var s_backdrop: sampler;"));
        assert!(source.contains("fn sample_backdrop("));
    }

    #[test]
    fn omits_backdrop_bindings_for_regular_shaders() {
        let source = shader_info(false).to_string();

        assert!(!source.contains("t_backdrop"));
        assert!(!source.contains("s_backdrop"));
        assert!(!source.contains("sample_backdrop"));
    }
}
