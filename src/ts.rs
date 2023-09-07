use crate::{
    js_ts::{self, ExportConfig},
    EventDataType, ExportLanguage, NoCommands, NoEvents, PluginBuilder,
};
use heck::ToLowerCamelCase;
use indoc::formatdoc;
use specta::{
    functions::FunctionDataType,
    ts::{self, TsExportError},
    TypeMap,
};
use tauri::Runtime;

/// Implements [`ExportLanguage`] for TypeScript exporting
pub struct Language;

pub fn builder<TRuntime: Runtime>() -> PluginBuilder<Language, NoCommands<TRuntime>, NoEvents> {
    PluginBuilder::default()
}

pub const GLOBALS: &str = include_str!("./globals.ts");

impl ExportLanguage for Language {
    /// Renders a collection of [`FunctionDataType`] into a TypeScript string.
    fn render_commands(
        commands: &[FunctionDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let commands = commands
            .iter()
            .map(|function| {
                let arg_defs = function
                    .args
                    .iter()
                    .map(|(name, typ)| {
                        ts::datatype(&cfg.inner, typ, type_map)
                            .map(|ty| format!("{}: {}", name.to_lower_camel_case(), ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let ret_type = js_ts::handle_result(function, type_map, cfg)?;

                Ok(js_ts::function(
                    &specta::ts::js_doc(&function.docs),
                    &function.name.to_lower_camel_case(),
                    &arg_defs,
                    Some(&ret_type),
                    &js_ts::command_body(cfg, function, true),
                ))
            })
            .collect::<Result<Vec<_>, TsExportError>>()?
            .join(",\n");

        Ok(formatdoc! {
            r#"
            export const commands = {{
            {commands}
            }}"#
        })
    }

    fn render_events(
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        if events.is_empty() {
            return Ok(Default::default());
        }

        let (events_types, events_map) = js_ts::events_data(events, cfg, type_map)?;

        let events_types = events_types.join(",\n");

        Ok(formatdoc! {
            r#"
            export const events = __makeEvents__<{{
            {events_types}
            }}>({{
            {events_map}
            }})"#
        })
    }

    fn render(
        commands: &[FunctionDataType],
        events: &[EventDataType],
        type_map: &TypeMap,
        cfg: &ExportConfig,
    ) -> Result<String, TsExportError> {
        let dependant_types = type_map
            .values()
            .filter_map(|v| v.as_ref())
            .map(|v| ts::export_named_datatype(&cfg.inner, v, type_map))
            .collect::<Result<Vec<_>, _>>()
            .map(|v| v.join("\n"))?;

        js_ts::render_all_parts::<Self>(commands, events, type_map, cfg, &dependant_types, GLOBALS)
    }
}
