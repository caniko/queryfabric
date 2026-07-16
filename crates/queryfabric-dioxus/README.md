# queryfabric-dioxus

Dioxus components for embedding the QueryFabric SyQL editor.

This crate is the Dioxus counterpart to `queryfabric-leptos`. It deliberately
keeps the existing textarea attributes, default values, and packaged script
URL unchanged so applications can switch renderers without changing the
QueryFabric web contract or JavaScript asset.

Applications opt into `web` or `server` explicitly. No renderer is enabled by
default.
