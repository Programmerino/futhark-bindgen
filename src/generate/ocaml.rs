use std::io::Write;

use crate::generate::first_uppercase;
use crate::*;

pub struct OCaml {
    typemap: BTreeMap<String, String>,
    ctypes_map: BTreeMap<String, String>,
    ba_map: BTreeMap<String, String>,
}

const OCAML_CTYPES_MAP: &[(&'static str, &'static str)] = &[
    ("i8", "int8_t"),
    ("u8", "uint8_t"),
    ("i16", "int16_t"),
    ("u16", "uint16_t"),
    ("i32", "int32_t"),
    ("u32", "uint32_t"),
    ("i64", "int64_t"),
    ("u64", "uint64_t"),
    ("f32", "float"),
    ("f64", "double"),
];

const OCAML_TYPE_MAP: &[(&'static str, &'static str)] = &[
    ("i8", "int"),
    ("u8", "int"),
    ("i16", "int"),
    ("u16", "int"),
    ("i32", "int32"),
    ("i64", "int64"),
    ("u32", "int32"),
    ("u64", "int64"),
    ("f32", "float"),
    ("f64", "float"),
];
const OCAML_BA_TYPE_MAP: &[(&'static str, &'static str)] = &[
    ("i8", "Bigarray.int8_signed_elt"),
    ("u8", "Bigarray.int8_unsigned_elt"),
    ("i16", "Bigarray.int16_signed_elt"),
    ("u16", "Bigarray.int16_unsigned_elt"),
    ("i32", "Bigarray.int32_elt"),
    ("i64", "Bigarray.int64_elt"),
    ("u32", "Bigarray.int32_elt"),
    ("u64", "Bigarray.int64_elt"),
    ("f32", "Bigarray.float32_elt"),
    ("f64", "Bigarray.float64_elt"),
];

fn type_is_array(t: &str) -> bool {
    t.contains("array_f") || t.contains("array_i") || t.contains("array_u") || t.contains("array_b")
}

fn type_is_opaque(t: &str) -> bool {
    t.contains(".t")
}

impl Default for OCaml {
    fn default() -> Self {
        let typemap = OCAML_TYPE_MAP
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();

        let ba_map = OCAML_BA_TYPE_MAP
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();

        let ctypes_map = OCAML_CTYPES_MAP
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();

        OCaml {
            typemap,
            ba_map,
            ctypes_map,
        }
    }
}

impl OCaml {
    fn foreign_function(&mut self, name: &str, ret: &str, args: Vec<&str>) -> String {
        format!(
            "let {name} = Foreign.foreign \"{name}\" ({} @-> returning ({ret}))",
            args.join(" @-> ")
        )
    }

    fn get_ctype(&self, t: &str) -> String {
        self.ctypes_map
            .get(t)
            .cloned()
            .unwrap_or_else(|| t.to_string())
    }

    fn get_type(&self, t: &str) -> String {
        self.typemap
            .get(t)
            .cloned()
            .unwrap_or_else(|| t.to_string())
    }

    fn get_ba_type(&self, t: &str) -> String {
        self.ba_map.get(t).cloned().unwrap_or_else(|| t.to_string())
    }
}

impl Generate for OCaml {
    fn generate(&mut self, library: &Library, config: &mut Config) -> Result<(), Error> {
        let mut mli_file = std::fs::File::create(config.output_path.with_extension("mli"))?;

        writeln!(mli_file, "(* Generated by futhark-bindgen *)\n")?;
        writeln!(config.output_file, "(* Generated by futhark-bindgen *)\n")?;

        let mut generated_foreign_functions = Vec::new();
        match library.manifest.backend {
            Backend::Multicore => {
                generated_foreign_functions.push(format!(
                    "  {}",
                    self.foreign_function(
                        "futhark_context_config_set_num_threads",
                        "void",
                        vec!["context_config", "int"]
                    )
                ));
            }
            Backend::CUDA | Backend::OpenCL => {
                generated_foreign_functions.push(format!(
                    "  {}",
                    self.foreign_function(
                        "futhark_context_config_set_device",
                        "void",
                        vec!["context_config", "string"]
                    )
                ));
            }
            _ => (),
        }

        for (name, ty) in &library.manifest.types {
            match ty {
                manifest::Type::Array(a) => {
                    let elemtype = a.elemtype.to_str().to_string();
                    let ctypes_elemtype = self.get_ctype(&elemtype);
                    let rank = a.rank;
                    let ocaml_name = format!("array_{elemtype}_{rank}d");
                    self.typemap.insert(name.clone(), ocaml_name.clone());
                    self.ctypes_map.insert(name.clone(), ocaml_name.clone());
                    let elem_ptr = format!("ptr {ctypes_elemtype}");
                    generated_foreign_functions.push(format!(
                        "  let {ocaml_name} = typedef (ptr void) \"array_{elemtype}_{rank}d\""
                    ));
                    let mut new_args = vec!["context", &elem_ptr];
                    for _ in 0..rank {
                        new_args.push("int64_t");
                    }
                    generated_foreign_functions.push(format!(
                        "  {}",
                        self.foreign_function(
                            &format!("futhark_new_{elemtype}_{rank}d"),
                            &ocaml_name,
                            new_args
                        )
                    ));
                    generated_foreign_functions.push(format!(
                        "  {}",
                        self.foreign_function(
                            &format!("futhark_values_{elemtype}_{rank}d"),
                            "int",
                            vec!["context", &ocaml_name, &elem_ptr]
                        )
                    ));
                    generated_foreign_functions.push(format!(
                        "  {}",
                        self.foreign_function(
                            &format!("futhark_free_{elemtype}_{rank}d"),
                            "int",
                            vec!["context", &ocaml_name]
                        )
                    ));
                    generated_foreign_functions.push(format!(
                        "  {}",
                        self.foreign_function(
                            &format!("futhark_shape_{elemtype}_{rank}d"),
                            "ptr int64_t",
                            vec!["context", &ocaml_name]
                        )
                    ));
                }
                manifest::Type::Opaque(ty) => {
                    generated_foreign_functions
                        .push(format!("  let {name} = typedef (ptr void) \"{name}\""));

                    let free_fn = &ty.ops.free;
                    generated_foreign_functions.push(format!(
                        "  {}",
                        self.foreign_function(free_fn, "int", vec!["context", name])
                    ));

                    let record = match &ty.record {
                        Some(r) => r,
                        None => continue,
                    };

                    let new_fn = &record.new;
                    let mut args = vec!["context".to_string(), format!("ptr {name}")];
                    for f in record.fields.iter() {
                        let cty = self
                            .ctypes_map
                            .get(&f.r#type)
                            .cloned()
                            .unwrap_or_else(|| f.r#type.clone());
                        args.push(cty);
                    }
                    let args = args.iter().map(|x| x.as_str()).collect();
                    generated_foreign_functions
                        .push(format!("  {}", self.foreign_function(new_fn, "int", args)));

                    for f in record.fields.iter() {
                        let cty = self
                            .ctypes_map
                            .get(&f.r#type)
                            .cloned()
                            .unwrap_or_else(|| f.r#type.clone());
                        generated_foreign_functions.push(format!(
                            "  {}",
                            self.foreign_function(
                                &f.project,
                                "int",
                                vec!["context", &format!("ptr {cty}"), name]
                            )
                        ));
                    }
                }
            }
        }

        for (_name, entry) in &library.manifest.entry_points {
            let mut args = vec!["context".to_string()];

            for out in &entry.outputs {
                let t = self.get_ctype(&out.r#type);

                args.push(format!("ptr {t}"));
            }

            for input in &entry.inputs {
                let t = self.get_ctype(&input.r#type);
                args.push(t);
            }

            let args = args.iter().map(|x| x.as_str()).collect();
            generated_foreign_functions.push(format!(
                "  {}",
                self.foreign_function(&entry.cfun, "int", args)
            ));
        }

        let generated_foreign_functions = generated_foreign_functions.join("\n");

        writeln!(
            config.output_file,
            include_str!("templates/ocaml/bindings.ml"),
            generated_foreign_functions = generated_foreign_functions
        )?;

        writeln!(mli_file, include_str!("templates/ocaml/bindings.mli"))?;

        let (extra_param, extra_line, extra_mli) = match library.manifest.backend {
            Backend::Multicore => (
                "?(num_threads = 0)",
                "    Bindings.futhark_context_config_set_num_threads config num_threads;",
                "?num_threads:int ->",
            ),

            Backend::CUDA | Backend::OpenCL => (
                "?device",
                "    Option.iter (Bindings.futhark_context_config_set_device config) device;",
                "?device:string ->",
            ),
            _ => ("", "", ""),
        };

        writeln!(
            config.output_file,
            include_str!("templates/ocaml/context.ml"),
            extra_param = extra_param,
            extra_line = extra_line
        )?;
        writeln!(
            mli_file,
            include_str!("templates/ocaml/context.mli"),
            extra_mli = extra_mli
        )?;

        for (name, ty) in &library.manifest.types {
            match ty {
                manifest::Type::Array(a) => {
                    let rank = a.rank;
                    let elemtype = a.elemtype.to_str().to_string();
                    let ocaml_name = self.typemap.get(name).unwrap();
                    let module_name = first_uppercase(&ocaml_name);
                    let mut dim_args = Vec::new();
                    for i in 0..rank {
                        dim_args.push(format!("(Int64.of_int dims.({i}))"));
                    }

                    let ocaml_elemtype = self.get_type(&elemtype);
                    let ba_elemtype = self.get_ba_type(&elemtype);

                    writeln!(
                        config.output_file,
                        include_str!("templates/ocaml/array.ml"),
                        module_name = module_name,
                        elemtype = elemtype,
                        rank = rank,
                        dim_args = dim_args.join(" ")
                    )?;

                    writeln!(
                        mli_file,
                        include_str!("templates/ocaml/array.mli"),
                        module_name = module_name,
                        ocaml_elemtype = ocaml_elemtype,
                        ba_elemtype = ba_elemtype,
                    )?;
                }
                manifest::Type::Opaque(ty) => {
                    let module_name = first_uppercase(name);
                    self.typemap
                        .insert(name.clone(), format!("{module_name}.t"));

                    let free_fn = &ty.ops.free;

                    let record = match &ty.record {
                        Some(r) => r,
                        None => {
                            writeln!(
                                config.output_file,
                                include_str!("templates/ocaml/opaque.ml"),
                                record_ml = "",
                                module_name = module_name,
                                free_fn = free_fn,
                                name = name,
                            )?;
                            writeln!(
                                mli_file,
                                include_str!("templates/ocaml/opaque.mli"),
                                record_mli = "",
                                module_name = module_name
                            )?;
                            continue;
                        }
                    };

                    let mut new_params = Vec::new();
                    let mut new_call_args = Vec::new();
                    let mut new_arg_types = Vec::new();
                    for f in record.fields.iter() {
                        let t = self.get_type(&f.r#type);

                        new_params.push(format!("field{}", f.name));

                        if type_is_array(&t) {
                            new_call_args.push(format!("field{}.ptr", f.name));
                            new_arg_types.push(format!("{}.t", first_uppercase(&t)));
                        } else if type_is_opaque(&t) {
                            new_call_args.push(format!("field{}.opaque_ptr", f.name));
                            new_arg_types.push(format!("{t}"));
                        } else {
                            new_call_args.push(format!("field{}", f.name));
                            new_arg_types.push(format!("{t}"));
                        }
                    }

                    let mut record_ml = format!(
                        include_str!("templates/ocaml/record.ml"),
                        new_params = new_params.join(" "),
                        new_fn = record.new,
                        new_call_args = new_call_args.join(" "),
                    );

                    let mut record_mli = format!(
                        include_str!("templates/ocaml/record.mli"),
                        new_arg_types = new_arg_types.join(" -> ")
                    );

                    for f in record.fields.iter() {
                        let t = self.get_type(&f.r#type);
                        let name = &f.name;
                        let project = &f.project;

                        let s = if type_is_array(&t) {
                            format!("Bindings.{t}")
                        } else {
                            t.clone()
                        };

                        let out = if type_is_opaque(&t) {
                            format!("of_raw t.opaque_ctx !@out")
                        } else if type_is_array(&t) {
                            let array = first_uppercase(&t);
                            format!("{array}.of_raw t.opaque_ctx !@out")
                        } else {
                            format!("!@out")
                        };

                        let out_type = if type_is_array(&t) {
                            format!("{}.t", first_uppercase(&t))
                        } else {
                            format!("{t}")
                        };

                        record_ml += &format!(
                            include_str!("templates/ocaml/record_project.ml"),
                            name = name,
                            s = s,
                            project = project,
                            out = out
                        );
                        record_mli += &format!(
                            include_str!("templates/ocaml/record_project.mli"),
                            name = name,
                            out_type = out_type
                        );
                    }

                    writeln!(
                        config.output_file,
                        include_str!("templates/ocaml/opaque.ml"),
                        record_ml = record_ml,
                        module_name = module_name,
                        free_fn = free_fn,
                        name = name,
                    )?;
                    writeln!(
                        mli_file,
                        include_str!("templates/ocaml/opaque.mli"),
                        record_mli = record_mli,
                        module_name = module_name
                    )?;
                }
            }
        }

        writeln!(config.output_file, "module Entry = struct")?;
        writeln!(mli_file, "module Entry: sig")?;

        for (name, entry) in &library.manifest.entry_points {
            let mut arg_types = Vec::new();
            let mut return_type = Vec::new();
            let mut entry_params = Vec::new();
            let mut call_args = Vec::new();
            let mut out_return = Vec::new();
            let mut out_decl = Vec::new();

            for (i, input) in entry.inputs.iter().enumerate() {
                entry_params.push(format!("input{i}"));

                let mut ocaml_elemtype = self.get_type(&input.r#type);

                // Transform into `Module.t`
                if type_is_array(&ocaml_elemtype) {
                    ocaml_elemtype = first_uppercase(&ocaml_elemtype) + ".t"
                }

                arg_types.push(ocaml_elemtype);
            }

            for (i, out) in entry.outputs.iter().enumerate() {
                let t = self.get_type(&out.r#type);
                let ct = self.get_ctype(&out.r#type);

                let mut ocaml_elemtype = t.clone();

                // Transform into `Module.t`
                if ocaml_elemtype.contains("array_") {
                    ocaml_elemtype = first_uppercase(&ocaml_elemtype) + ".t"
                }

                return_type.push(ocaml_elemtype);

                let i = if entry.outputs.len() == 1 {
                    String::new()
                } else {
                    format!("{i}")
                };

                if type_is_array(&t) {
                    out_decl.push(format!(
                        "    let out{i}_ptr = allocate_n (ptr void) ~count:1 in"
                    ));
                } else if type_is_opaque(&t) {
                    out_decl.push(format!(
                        "    let out{i}_ptr = allocate_n (ptr void) ~count:1 in"
                    ));
                } else {
                    out_decl.push(format!("    let out{i}_ptr = allocate_n {ct} ~count:1 in"));
                }
            }

            for (i, _out) in entry.outputs.iter().enumerate() {
                let i = if entry.outputs.len() == 1 {
                    String::new()
                } else {
                    format!("{i}")
                };
                call_args.push(format!("out{i}_ptr"));
            }

            for (i, input) in entry.inputs.iter().enumerate() {
                let t = self.get_type(&input.r#type);
                if type_is_array(&t) {
                    call_args.push(format!("input{i}.ptr"));
                } else if type_is_opaque(&t) {
                    call_args.push(format!("input{i}.opaque_ptr"));
                } else {
                    call_args.push(format!("input{i}"));
                }
            }

            for (i, out) in entry.outputs.iter().enumerate() {
                let t = self.get_type(&out.r#type);

                let idx = if entry.outputs.len() == 1 {
                    String::new()
                } else {
                    format!("{i}")
                };

                if type_is_array(&t) {
                    let m = first_uppercase(&t);
                    out_return.push(format!("({m}.of_raw ctx !@out{idx}_ptr)"));
                } else if type_is_opaque(&t) {
                    let m = first_uppercase(&t);
                    let m = m.strip_suffix(".t").unwrap_or(&m);
                    out_return.push(format!("({m}.of_raw ctx !@out{idx}_ptr)"));
                } else {
                    out_return.push(format!("!@out{idx}_ptr"));
                }
            }
            writeln!(
                config.output_file,
                include_str!("templates/ocaml/entry.ml"),
                name = name,
                entry_params = entry_params.join(" "),
                out_decl = out_decl.join("\n"),
                call_args = call_args.join(" "),
                out_return = out_return.join(", ")
            )?;
            writeln!(
                mli_file,
                include_str!("templates/ocaml/entry.mli"),
                name = name,
                arg_types = arg_types.join(" -> "),
                return_type = return_type.join(", "),
            )?;
        }
        writeln!(config.output_file, "end")?;
        writeln!(mli_file, "end")?;

        Ok(())
    }
}
