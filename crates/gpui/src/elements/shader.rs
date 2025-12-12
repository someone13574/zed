use std::marker::PhantomData;

use smallvec::SmallVec;

use crate::{
    App, Bounds, CursorStyle, Edges, Element, ElementId, GlobalElementId, Hitbox,
    InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId, Pixels,
    SharedString, StyleRefinement, Window, fill, point, px, rgb,
};

/// A custom shader which can be drawn using [shader_element] or [shader_element_with_data].
#[derive(Clone)]
pub struct FragmentShader<T: ShaderUniform> {
    main_body: SharedString,
    extra_items: SmallVec<[SharedString; 4]>,
    read_access: bool,
    read_margin: Option<Edges<Pixels>>,
    _marker: PhantomData<T>,
}

impl<T: ShaderUniform> FragmentShader<T> {
    /// Create a new fragment shader.
    ///
    /// The `main_body` contains the body of the fragment shaders function,
    /// written in [WGSL](https://www.w3.org/TR/WGSL/). This code *must* return
    /// a `vec4<f32>` containing the color for that pixels in RGBA, with values
    /// from 0 to 1.
    ///
    /// Within this function, you have access to the following parameters:
    ///
    /// - `position` (`vec2<f32>`): The absolute position of the pixel within
    ///   the window. The units are in logical pixels, *not* device pixels.
    /// - `bounds` (`Bounds { origin: vec2<f32>, size: vec2<f32> }`): The bounds
    ///   of this shader, in the same units as `position`.
    /// - `scale_factor` (`f32`): See [Window::scale_factor()]. This can be used
    ///   to convert to device pixels.
    /// - `data`: This value will only be present if drawn using [shader_element_with_data].
    ///   Its type is whatever type the instance data is.
    /// - `globals.viewport_size` (`vec2<f32>`): The size of the surface in
    ///   *device pixels*. You will need to divide by `scale_factor` if you
    ///   require logical pixels.
    ///
    /// Additionally, any functions or types defined using [FragmentShader::with_item]
    /// will be accessible within the main body.
    pub fn new(main_body: &'static str) -> Self {
        Self {
            main_body: SharedString::new_static(main_body),
            extra_items: SmallVec::new(),
            read_access: false,
            read_margin: Some(Edges::all(px(0.0))),
            _marker: PhantomData,
        }
    }

    /// Adds a helper function or type to the shader code.
    pub fn with_item(mut self, item: &'static str) -> Self {
        self.extra_items.push(SharedString::new_static(item));
        self
    }

    /// Gives this shader read access to the pixels within its bounds. You can
    /// sample pixels using the `sample_backdrop` function, or by using
    /// `t_backdrop` and `s_backdrop`.
    ///
    /// ```
    /// /// Samples the pixel at `position` (in absolute logical pixels)
    /// /// and returns the color in RGBA.
    /// fn sample_backdrop(position: vec2<f32>, scale_factor: f32) -> vec4<f32>;
    /// ```
    ///
    /// Sampling outside of the instance's bounds may result in unexpected behavior.
    /// Use [FragmentShader::read_margin] and [FragmentShader::read_full] to
    /// expand the valid area.
    pub fn read_under(mut self) -> Self {
        self.read_access = true;
        self
    }

    /// Gives this shader read access to the pixels within its bounds + `margin`.
    /// You can sample pixels using the `sample_backdrop` function, or by using
    /// `t_backdrop` and `s_backdrop`.
    ///
    /// ```
    /// /// Samples the pixel at `position` (in absolute logical pixels)
    /// /// and returns the color in RGBA.
    /// fn sample_backdrop(position: vec2<f32>, scale_factor: f32) -> vec4<f32>;
    /// ```
    ///
    /// Sampling outside of valid area may result in unexpected behavior. Use
    /// [FragmentShader::read_full] to give access to all pixels within the
    /// window.
    pub fn read_margin(mut self, margin: Edges<Pixels>) -> Self {
        self.read_access = true;
        self.read_margin = Some(margin);
        self
    }

    /// Gives this shader read access to the pixels within the window.
    /// You can sample pixels using the `sample_backdrop` function, or by using
    /// `t_backdrop` and `s_backdrop`.
    ///
    /// ```
    /// /// Samples the pixel at `position` (in absolute logical pixels)
    /// /// and returns the color in RGBA.
    /// fn sample_backdrop(position: vec2<f32>, scale_factor: f32) -> vec4<f32>;
    /// ```
    pub fn read_full(mut self) -> Self {
        self.read_access = true;
        self.read_margin = None;
        self
    }
}

/// An element which can render an instance of a fragment shader.
/// Use [shader_element] or [shader_element_with_data] to construct.
pub struct ShaderElement<T: ShaderUniform, const PASSES: usize> {
    shader: FragmentShader<T>,
    data: [T; PASSES],
    interactivity: Interactivity,
}

impl<T: ShaderUniform, const PASSES: usize> ShaderElement<T, PASSES> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }

    gpui::visibility_style_methods!({
        visibility: pub
    });

    gpui::margin_style_methods!({
        visibility: pub
    });

    gpui::position_style_methods!({
        visibility: pub
    });

    gpui::size_style_methods!({
        visibility: pub
    });

    gpui::cursor_style_methods!({
        visibility: pub
    });
}

impl<T: ShaderUniform, const PASSES: usize> InteractiveElement for ShaderElement<T, PASSES> {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

/// Constructs a [ShaderElement] which renders a shader which *doesn't* take
/// instance data. If you need to pass data to your shader, use [shader_element_with_data].
pub fn shader_element(shader: FragmentShader<()>) -> ShaderElement<(), 1> {
    ShaderElement {
        shader,
        data: [()],
        interactivity: Interactivity::new(),
    }
}

/// Constructs a [ShaderElement] which renders the shader while exposing `data`
/// within the shader's main body. If the data array contains multiple instances,
/// then the shader will be run once for each element in that array, using the
/// same bounds.
pub fn shader_element_with_data<T: ShaderUniform, const PASSES: usize>(
    shader: FragmentShader<T>,
    data: [T; PASSES],
) -> ShaderElement<T, PASSES> {
    ShaderElement {
        shader,
        data,
        interactivity: Interactivity::new(),
    }
}

impl<T: ShaderUniform, const PASSES: usize> IntoElement for ShaderElement<T, PASSES> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<T: ShaderUniform, const PASSES: usize> Element for ShaderElement<T, PASSES> {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| window.request_layout(style, None, cx),
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, _, _| hitbox,
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |_style, window, _cx| match window.register_shader::<T>(
                self.shader.main_body.clone(),
                self.shader.extra_items.clone(),
                self.shader.read_access,
            ) {
                Ok(shader_id) => {
                    for pass_data in &self.data {
                        window.paint_shader(
                            shader_id,
                            bounds,
                            self.shader.read_access,
                            self.shader.read_margin,
                            pass_data,
                        );
                    }
                }
                Err((msg, first_err)) => {
                    paint_error_texture(bounds, window);

                    if first_err {
                        eprintln!("Shader compile error: {msg}");
                    }
                }
            },
        );
    }
}

fn paint_error_texture(bounds: Bounds<Pixels>, window: &mut Window) {
    for x in 0..5 {
        for y in 0..5 {
            window.paint_quad(fill(
                Bounds {
                    origin: bounds.origin
                        + point(bounds.size.width / 5.0 * x, bounds.size.height / 5.0 * y),
                    size: bounds.size / 5.0,
                },
                if (x + y) & 1 == 0 {
                    rgb(0xff00ff)
                } else {
                    rgb(0x000000)
                },
            ));
        }
    }
}

/// Marker trait for data which can be passed to custom WGSL shaders.
///
/// To create a custom structure, use the derive macro [derive@crate::ShaderUniform]:
///
/// ```rust
/// #[repr(C)]
/// #[derive(gpui::ShaderUniform, Clone, Copy)]
/// struct MyStruct {
///     color: [f32; 4],
///     something: u32,
/// }
/// ```
///
/// SAFETY: If implementing this trait manually (*not* through the derive macro),
/// then you must ensure that the definitions in both languages are compatible
/// and that alignment is correct. If alignment is incorrect or the field
/// ordering does not match the definition, then the shader may fail to compile
/// or you may get unexpected behavior. Also ensure that your type is `#[repr(C)]`
/// to ensure it has a defined layout.
pub unsafe trait ShaderUniform: Clone + Copy + 'static {
    /// The name of the type in WGSL (eg. `f32`, `MyStruct`).
    const NAME: &str;

    /// The type's definition, if it requires one (eg. a struct). This will be
    /// included in the shader's source code.
    const DEFINITION: Option<&str>;

    /// The [WGSL alignment](https://sotrh.github.io/learn-wgpu/showcase/alignment/#alignment-of-uniform-and-storage-buffers)
    /// of this type in bytes.
    const ALIGN: usize;
}

// Only used to mark instance data as unused. The derive macro will prevent it from being used.
unsafe impl ShaderUniform for () {
    const NAME: &str = "This shouldn't ever be emitted";
    const DEFINITION: Option<&str> = None;
    const ALIGN: usize = 1;
}

macro_rules! impl_scalar {
    ($ty:ty, $name:literal) => {
        unsafe impl ShaderUniform for $ty {
            const NAME: &str = $name;
            const DEFINITION: Option<&str> = None;
            const ALIGN: usize = 4;
        }

        unsafe impl ShaderUniform for [$ty; 2] {
            const NAME: &str = concat!("vec2<", $name, ">");
            const DEFINITION: Option<&str> = None;
            const ALIGN: usize = 8;
        }

        unsafe impl ShaderUniform for [$ty; 3] {
            const NAME: &str = concat!("vec3<", $name, ">");
            const DEFINITION: Option<&str> = None;
            const ALIGN: usize = 16;
        }

        unsafe impl ShaderUniform for [$ty; 4] {
            const NAME: &str = concat!("vec4<", $name, ">");
            const DEFINITION: Option<&str> = None;
            const ALIGN: usize = 16;
        }
    };
}

impl_scalar!(u32, "u32");
impl_scalar!(i32, "i32");
impl_scalar!(f32, "f32");
