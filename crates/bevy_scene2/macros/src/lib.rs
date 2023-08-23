mod bsn;

use crate::bsn::BsnScene;
use bevy_macro_utils::BevyManifest;
use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, Path, PathArguments, Type};

pub(crate) fn bevy_ecs_path() -> syn::Path {
    BevyManifest::default().get_path("bevy_ecs")
}

pub(crate) fn bevy_scene_path() -> syn::Path {
    BevyManifest::default().get_path("bevy_scene2")
}

pub(crate) fn bevy_bsn_path() -> syn::Path {
    BevyManifest::default().get_path("bevy_bsn")
}

const SCHEMATIC_ATTRIBUTE: &str = "schematic";

#[proc_macro_derive(Schematic, attributes(schematic))]
pub fn derive_schematic(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let bevy_scene_path: Path = bevy_scene_path();
    let bevy_bsn_path: Path = bevy_bsn_path();

    let mut from_props_field_assignments = Vec::new();
    let mut props_fields = Vec::new();
    let mut props_impl_field_applies = Vec::new();
    let mut from_bsn_fields = Vec::new();
    let mut schematic_system = None;
    for attr in ast.attrs.iter() {
        if attr.path().is_ident(SCHEMATIC_ATTRIBUTE) {
            schematic_system = Some(attr.parse_args::<Ident>().unwrap());
        }
    }

    // TODO: schematic_system impl needs some serious work
    if let Some(system_ident) = schematic_system {
        if let Data::Struct(data_struct) = &ast.data {
            let is_named = matches!(data_struct.fields, Fields::Named(_));
            for field in data_struct.fields.iter() {
                let ident = &field.ident;
                let ty = &field.ty;
                if is_named {
                    // TODO: this bit is extremely hackey
                    let inside_type = if let Type::Path(ty_path) = &ty {
                        if let PathArguments::AngleBracketed(args) =
                            &ty_path.path.segments.first().as_ref().unwrap().arguments
                        {
                            args.args.first().unwrap()
                        } else {
                            panic!()
                        }
                    } else {
                        panic!()
                    };
                    let field_name = ident.as_ref().unwrap().to_string();
                    from_bsn_fields.push(quote! {
                            #field_name => props.#ident = #bevy_scene_path::Prop::Value(<#inside_type as #bevy_bsn_path::FromBsn>::from_bsn(field.value)?),
                        });
                } else {
                    todo!("Unnamed fields are not supported yet");
                }
            }
        } else {
            todo!("Non-struct types are not supported yet.");
        }

        let struct_name = &ast.ident;
        let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

        let state_type = format_ident!("{struct_name}State");

        let bevy_ecs_path = bevy_ecs_path();

        TokenStream::from(quote! {
            impl #impl_generics #bevy_scene_path::Schematic for #struct_name #type_generics #where_clause {
                type Props = Self;

                fn from_props(
                    props: Self::Props,
                    context: &mut #bevy_scene_path::SchematicContext,
                ) -> Result<Self, #bevy_scene_path::SchematicError> {
                    context.entity.world.init_resource::<#state_type>();
                    let bsn = context
                        .entity
                        .world
                        .resource_scope(|world, mut state: Mut<#state_type #type_generics>| {
                            state.system.run(props.clone(), world)
                        });
                    if bsn.scene_paths().is_empty() {
                        bsn.apply(context)?;
                    }
                    Ok(props)
                }
            }

            #[derive(Resource)]
            pub struct #state_type #type_generics #where_clause  {
                system: Box<dyn System<In = #struct_name #type_generics, Out = Bsn>>,
            }

            impl #bevy_ecs_path::world::FromWorld for #state_type {
                fn from_world(world: &mut #bevy_ecs_path::world::World) -> Self {
                    let mut system = #bevy_ecs_path::system::IntoSystem::into_system(#system_ident);
                    system.initialize(world);
                    Self {
                        system: Box::new(system),
                    }
                }
            }

            impl #impl_generics #bevy_bsn_path::FromBsn for #struct_name #type_generics #where_clause  {
                fn from_bsn<'a>(value: #bevy_bsn_path::BsnValue<'a>) -> Result<Self, #bevy_bsn_path::FromBsnError> {
                    let mut props = Self::default();
                    match value {
                        #bevy_bsn_path::BsnValue::Struct(#bevy_bsn_path::BsnStruct::NamedFields(fields)) => {
                            for field in fields {
                                match field.name {
                                    #(#from_bsn_fields)*
                                    field => return Err(#bevy_bsn_path::FromBsnError::UnexpectedField(field.to_string())),
                                }
                            }
                        }
                        #bevy_bsn_path::BsnValue::Struct(#bevy_bsn_path::BsnStruct::Tuple(fields)) => {
                            if !fields.is_empty() {
                                return Err(#bevy_bsn_path::FromBsnError::MismatchedType)
                            }
                        }
                        _ => return Err(#bevy_bsn_path::FromBsnError::MismatchedType)
                    }
                    Ok(props)
                }
            }

            impl #bevy_scene_path::Props for #struct_name #type_generics #where_clause {
                fn apply_props(&mut self, other: &Self) {
                    *self = other.clone();
                }
            }
        })
    } else {
        if let Data::Struct(data_struct) = &ast.data {
            let is_named = matches!(data_struct.fields, Fields::Named(_));
            for field in data_struct.fields.iter() {
                let ident = &field.ident;
                let ty = &field.ty;
                props_impl_field_applies.push(quote! {
                    self.#ident.apply(&other.#ident);
                });
                let mut attrs = field.attrs.iter();
                let is_schematic = attrs.any(|a| a.path().is_ident(SCHEMATIC_ATTRIBUTE));
                if is_schematic {
                    from_props_field_assignments.push(quote! {
                        #ident: #bevy_scene_path::Schematic::from_props(props.#ident.get(), context)?,
                    });
                    if is_named {
                        let field_name = ident.as_ref().unwrap().to_string();
                        from_bsn_fields.push(quote! {
                            #field_name => props.#ident = #bevy_scene_path::Prop::Value(<<#ty as #bevy_scene_path::Schematic>::Props as #bevy_bsn_path::FromBsn>::from_bsn(field.value)?),
                        });
                    } else {
                        todo!("Unnamed fields are not supported yet");
                    }
                    props_fields.push(quote! {
                        pub #ident: #bevy_scene_path::Prop<<#ty as #bevy_scene_path::Schematic>::Props>,
                    });
                } else {
                    if is_named {
                        let field_name = ident.as_ref().unwrap().to_string();
                        from_bsn_fields.push(quote! {
                            #field_name => props.#ident = #bevy_scene_path::Prop::Value(<#ty as #bevy_bsn_path::FromBsn>::from_bsn(field.value)?),
                        });
                    } else {
                        todo!("Unnamed fields are not supported yet");
                    }
                    from_props_field_assignments.push(quote! {#ident: props.#ident.get(),});
                    props_fields.push(quote! {
                        pub #ident: #bevy_scene_path::Prop<#ty>,
                    });
                }
            }
        } else {
            todo!("Non-struct types are not supported yet.");
        }

        let struct_name = &ast.ident;
        let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

        let props_type = format_ident!("{struct_name}Props");

        TokenStream::from(quote! {
            impl #impl_generics #bevy_scene_path::Schematic for #struct_name #type_generics #where_clause {
                type Props = #props_type #type_generics #where_clause;

                fn from_props(
                    props: Self::Props,
                    context: &mut #bevy_scene_path::SchematicContext,
                ) -> Result<Self, #bevy_scene_path::SchematicError> {
                    Ok(Self {
                        #(#from_props_field_assignments)*
                    })
                }
            }

            #[derive(Clone, Default, Reflect)]
            pub struct #props_type #type_generics #where_clause {
                #(#props_fields)*
            }

            impl #impl_generics #bevy_bsn_path::FromBsn for #props_type #type_generics #where_clause  {
                fn from_bsn<'a>(value: #bevy_bsn_path::BsnValue<'a>) -> Result<Self, #bevy_bsn_path::FromBsnError> {
                    let mut props = Self::default();
                    match value {
                        #bevy_bsn_path::BsnValue::Struct(#bevy_bsn_path::BsnStruct::NamedFields(fields)) => {
                            for field in fields {
                                match field.name {
                                    #(#from_bsn_fields)*
                                    field => return Err(#bevy_bsn_path::FromBsnError::UnexpectedField(field.to_string())),
                                }
                            }
                        }
                        #bevy_bsn_path::BsnValue::Struct(#bevy_bsn_path::BsnStruct::Tuple(fields)) => {
                            if !fields.is_empty() {
                                return Err(#bevy_bsn_path::FromBsnError::MismatchedType)
                            }
                        }
                        _ => return Err(#bevy_bsn_path::FromBsnError::MismatchedType)
                    }
                    Ok(props)
                }
            }

            impl #bevy_scene_path::Props for #props_type #type_generics #where_clause {
                fn apply_props(&mut self, other: &Self) {
                    #(#props_impl_field_applies)*
                }
            }
        })
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn bsn(input: TokenStream) -> TokenStream {
    let scene = parse_macro_input!(input as BsnScene);
    TokenStream::from(scene.bsn_code_tokens())
}
