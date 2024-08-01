use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::Write,
    path::Path,
};

use serde::Serialize;
use specta::{
    datatype::{DataType, Function},
    NamedType, SpectaID, Type, TypeMap,
};
use tauri::{ipc::Invoke, Manager, Runtime};

use crate::{Commands, EventRegistry, Events, LanguageExt};

/// Builder for configuring Tauri Specta in your application.
///
/// # Example
///
/// You can copy the following code into your `main.rs` file to get started with Tauri Specta.
///
/// This will automatically export a [Typescript](https://www.typescriptlang.org) file containing bindings for all of your commands and events.
///
/// You can extend this example by calling other methods on the builder to configure your application further.
///
/// ```rust
/// let mut builder = <Builder>::new()
///     .commands(tauri_specta::collect_commands![]);
///
/// #[cfg(debug_assertions)]
/// builder
///     .export(Typescript::default().path("../src/bindings.ts"))
///     .expect("Failed to export typescript bindings");
///
/// tauri::Builder::default()
///     .invoke_handler(builder.invoke_handler()) // < Required for commands to work
///     .setup(move |app| {
///         builder.mount_events(app); // < Required for events to work
///
///         Ok(())
///     })
///     .run(tauri::generate_context!())
///     .expect("error while running tauri application");
/// ```
///
/// # Exporting using JSDoc
///
/// ```rs
/// # TODO
/// ```
pub struct Builder<R: Runtime = tauri::Wry> {
    // TODO: Can we just hold a `ExportContext` here to make it a bit neater???
    plugin_name: Option<&'static str>,
    commands: Commands<R>,
    command_types: Vec<Function>,
    events: BTreeMap<&'static str, DataType>,
    event_sids: BTreeSet<SpectaID>,
    types: TypeMap,
    constants: HashMap<Cow<'static, str>, serde_json::Value>,
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self {
            plugin_name: None,
            commands: Commands::default(),
            command_types: Default::default(),
            events: Default::default(),
            event_sids: Default::default(),
            types: TypeMap::default(),
            constants: HashMap::default(),
        }
    }
}

impl<R: Runtime> Builder<R> {
    /// Construct a new Tauri Specta builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the name of the current plugin name.
    ///
    /// This is used to ensure the generated bindings correctly reference the plugin.
    pub fn plugin_name(self, plugin_name: &'static str) -> Self {
        Self {
            plugin_name: Some(plugin_name),
            ..self
        }
    }

    /// Register commands with the builder.
    ///
    /// **WARNING:** This method will overwrite any previously registered commands.
    pub fn commands(mut self, commands: Commands<R>) -> Self {
        Self {
            command_types: (commands.1)(&mut self.types),
            commands,
            ..self
        }
    }

    /// Register events with the builder.
    ///
    /// **WARNING:** This method will overwrite any previously registered events.
    pub fn events(mut self, events: Events) -> Self {
        let mut event_sids = BTreeSet::new();
        let events = events
            .0
            .iter()
            .map(|(k, build)| {
                let (sid, dt) = build(&mut self.types);
                event_sids.insert(sid);
                (*k, dt)
            })
            .collect();

        // TODO: This must remain feature gated until this PR is merged and released: https://github.com/tauri-apps/tauri/pull/10435
        #[cfg(feature = "UNSTABLE_channels")]
        self.types
            .remove(<tauri::ipc::Channel<()> as specta::NamedType>::sid());

        Self {
            events,
            event_sids,
            ..self
        }
    }
    /// Export a new type with the frontend.
    ///
    /// This is useful if you want to export types that do not appear in any events or commands.
    pub fn ty<T: NamedType>(mut self) -> Self {
        let dt = T::definition_named_data_type(&mut self.types);
        self.types.insert(T::sid(), dt);
        self
    }

    /// Export a constant value to the frontend.
    ///
    /// This is useful to share application-wide constants or expose data which is generated by Rust.
    #[track_caller]
    pub fn constant<T: Serialize + Type>(mut self, k: impl Into<Cow<'static, str>>, v: T) -> Self {
        self.constants.insert(
            k.into(),
            serde_json::to_value(v).expect("Tauri Specta failed to serialize constant"),
        );
        self
    }

    // TODO: Maybe method to merge in a `TypeCollection`

    // TODO: Should we put a `.build` command here to ensure it's immutable from now on?

    /// The Tauri invoke handler to trigger commands registered with the builder.
    pub fn invoke_handler(&self) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
        let commands = self.commands.0.clone();
        move |invoke| commands(invoke)
    }

    /// Mount all of the events in the builder onto a Tauri app.
    pub fn mount_events(&self, handle: &impl Manager<R>) {
        if !handle.manage(EventRegistry {
            plugin_name: self.plugin_name,
            events: self.event_sids.clone(),
        }) {
            panic!("Attempted to mount Tauri Specta EventRegistry more than once. Did you call `Builder::mount_events` more than once?");
        }
    }

    /// Export the bindings to a string.
    ///
    /// # Example
    /// ```rust
    /// # TODO
    /// ```
    pub fn export_str<L: LanguageExt>(&self, language: L) -> Result<String, L::Error> {
        // TODO: Handle duplicate type names
        // TODO: Serde checking

        language.render(&crate::ExportContext {
            // TODO: Don't clone stuff
            commands: self.command_types.clone(),
            events: self.events.clone(),
            type_map: self.types.clone(),
            constants: self.constants.clone(),
            plugin_name: self.plugin_name,
        })
    }

    /// Export the bindings to a file.
    ///
    /// # Example
    /// ```rust
    /// # TODO
    /// ```
    pub fn export<L: LanguageExt>(
        &self,
        language: L,
        path: impl AsRef<Path>,
    ) -> Result<(), L::Error> {
        let path = path.as_ref();
        if let Some(export_dir) = path.parent() {
            fs::create_dir_all(export_dir)?;
        }

        let mut file = File::create(&path)?;
        write!(file, "{}", self.export_str(&language)?)?;
        language.format(path).ok(); // TODO: Error handling

        Ok(())
    }
}