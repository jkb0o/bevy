use crate::bevy_scene_path;
use quote::{format_ident, quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::token::{Bracket, Colon, Paren};
use syn::{braced, bracketed, parenthesized, Block, Ident, Lit, LitStr, Token};
use syn::{token::Brace, Path, Result};

pub struct BsnScene {
    root: BsnEntity,
}

impl BsnScene {
    pub fn bsn_code_tokens(&self) -> proc_macro2::TokenStream {
        let bevy_scene = bevy_scene_path();
        let mut scene_paths = Vec::new();
        let root_bsn = self.root.bsn_code_tokens(&bevy_scene, &mut scene_paths, 0);

        quote! {
            #bevy_scene::Bsn::new(
                Box::new(
                    move |context: &mut #bevy_scene::SchematicContext, loaded_scenes: &[Handle<#bevy_scene::Scene>]| {
                        #root_bsn
                        Ok(())
                    }
                ),
                vec![#(#scene_paths.to_string()),*]
            )
        }
    }
}

impl Parse for BsnScene {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            root: BsnEntity::parse(input)?,
        })
    }
}

pub struct BsnEntity {
    configs: Vec<BsnEntityConfig>,
    children: Vec<BsnEntity>,
}

impl BsnEntity {
    fn bsn_code_tokens(
        &self,
        bevy_scene: &Path,
        scene_paths: &mut Vec<String>,
        scene_path_offset: usize,
    ) -> proc_macro2::TokenStream {
        let mut schematic_len: usize = 0;
        let mut schematic_field_len: usize = 0;
        let mut scene_len: usize = 0;
        for config in self.configs.iter() {
            match config {
                BsnEntityConfig::Schematic { fields, .. } => {
                    schematic_len += 1;
                    schematic_field_len += fields.len()
                }
                BsnEntityConfig::Scene(_) => {
                    scene_len += 1;
                }
            }
        }

        // NOTE: These capacities should always line up exactly to avoid reallocations
        let mut schematic_props = Vec::with_capacity(schematic_len + schematic_field_len);
        let mut schematic_from_props = Vec::with_capacity(schematic_len);
        let mut schematic_paths = Vec::with_capacity(schematic_len);
        let mut schematic_idents = Vec::with_capacity(schematic_len);
        let mut prop_idents = Vec::with_capacity(schematic_len);
        let mut scene_blocks = Vec::with_capacity(scene_len);

        let mut schematic_index: usize = 0;
        let mut scene_index: usize = 0;
        for config in self.configs.iter() {
            match config {
                BsnEntityConfig::Schematic { path, fields } => {
                    schematic_paths.push(path);
                    let prop_ident = format_ident!("p{}", schematic_index);
                    schematic_props.push(quote! {
                        let mut #prop_ident = <<#path as #bevy_scene::Schematic>::Props as Default>::default();
                    });

                    for field in fields {
                        let field_name = &field.name;
                        let field_value = &field.value;
                        schematic_props.push(quote! {
                            #prop_ident.#field_name = #bevy_scene::Prop::Value(#field_value);
                        });
                    }

                    let schematic_ident = format_ident!("s{}", schematic_index);
                    schematic_from_props.push(quote! {
                        let #schematic_ident = <#path as #bevy_scene::Schematic>::from_props(#prop_ident, context)?;
                    });
                    prop_idents.push(prop_ident);
                    schematic_idents.push(schematic_ident);
                    schematic_index += 1;
                }
                BsnEntityConfig::Scene(path) => {
                    let loaded_scene_index = scene_path_offset + scene_index;
                    let mut applied_schematics = Vec::with_capacity(schematic_index);
                    for index in 0..schematic_index {
                        let path = schematic_paths[index];
                        let prop_ident = &prop_idents[index];
                        applied_schematics.push(quote! {
                            // TODO: we should skip applying scene schematics that will be overwritten by this BSN
                            if scene.root.apply_to_props::<#path, <#path as #bevy_scene::Schematic>::Props>(&mut #prop_ident) {
                                // skips.insert(std::any::TypeId::of::<#path>());
                            }
                        })
                    }
                    scene_paths.push(path.value());
                    scene_blocks.push(quote! {{
                        let scene = context.scenes.get(&loaded_scenes[#loaded_scene_index]).unwrap();
                        // let mut skips = bevy::utils::HashSet::new();
                        #(#applied_schematics)*
                        scene.apply(context)?;
                    }});
                    scene_index += 1;
                }
            }
        }

        // TODO: This behavior doesn't _quite_ line up with the Scene type behavior, as scenes are applied "last". Allowing
        // interleaving would likely require being more "dynamic" (more TypeId hashmaps), which feels suboptimal for scenes
        // defined in code. We want this macro to be as efficient as possible / roughly equivalent to spawning entities/components
        // manually

        // Scenes are applied in the reverse order that they are defined
        let scene_blocks = scene_blocks.iter().rev();

        let children = self.children.iter().map(|c| {
            let child_bsn =
                c.bsn_code_tokens(bevy_scene, scene_paths, scene_path_offset + scene_index);
            // PERF: investigate allocations here. this might be wasteful / might slow down compilation
            quote! {
                context.spawn_child(|context| {
                    #child_bsn
                    Ok(())
                })?;
            }
        });

        quote! {
            #(#schematic_props)*
            #(#scene_blocks)*
            #(#schematic_from_props)*
            context.entity.get()?.insert((
                #(#schematic_idents),*
            ));
            #(#children)*
        }
    }
}

impl Parse for BsnEntity {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut schematics = Vec::new();
        if input.peek(Paren) {
            let content;
            parenthesized![content in input];
            while !content.is_empty() {
                schematics.push(content.parse::<BsnEntityConfig>()?);
            }
        } else {
            schematics.push(input.parse::<BsnEntityConfig>()?);
        }

        let mut children = Vec::new();
        if input.peek(Bracket) {
            let content;
            bracketed![content in input];
            while !content.is_empty() {
                let child = content.parse::<BsnEntity>()?;
                children.push(child);
            }
        }
        Ok(Self {
            configs: schematics,
            children,
        })
    }
}

pub enum BsnEntityConfig {
    Schematic { path: Path, fields: Vec<BsnField> },
    Scene(LitStr),
}

impl Parse for BsnEntityConfig {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(if input.peek(Token![@]) {
            input.parse::<Token![@]>()?;
            let path = input.parse::<LitStr>()?;
            BsnEntityConfig::Scene(path)
        } else {
            let path = input.parse::<Path>()?;
            let mut fields = Vec::new();
            if input.peek(Brace) {
                let content;
                braced![content in input];
                while !content.is_empty() {
                    let name = content.parse::<Ident>()?;
                    let value = if content.peek(Colon) {
                        content.parse::<Colon>()?;
                        if content.peek(Brace) {
                            // This approach has limitations for autocomplete because typing things like `.` break expressions
                            Some(BsnValue::Expr(content.parse::<Block>()?))
                        } else if content.peek(Ident) {
                            Some(BsnValue::Ident(content.parse::<Ident>()?))
                        } else if content.peek(Lit) {
                            Some(BsnValue::Lit(content.parse::<Lit>()?))
                        } else {
                            todo!()
                        }
                    } else {
                        None
                    };
                    fields.push(BsnField { name, value })
                }
            }
            BsnEntityConfig::Schematic { path, fields }
        })
    }
}

pub struct BsnField {
    name: Ident,
    /// This is an Option to enable autocomplete when the field name is being typed
    /// To improve autocomplete further we'll need to forgo a lot of the syn parsing
    value: Option<BsnValue>,
}

#[derive(Debug)]
pub enum BsnValue {
    Expr(Block),
    Ident(Ident),
    Lit(Lit),
}

impl ToTokens for BsnValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let result = match self {
            BsnValue::Expr(expr) => {
                let statements = &expr.stmts;
                quote! {#(#statements)*}
            }
            BsnValue::Ident(ident) => quote! {#ident},
            BsnValue::Lit(lit) => match lit {
                Lit::Str(str) => quote! {#str.to_string().into()},
                _ => quote! {#lit},
            },
        };
        result.to_tokens(tokens);
    }
}
