# Included data and parser notices

Native PIM uses the `ical` Rust crate (v0.11, Apache-2.0) for vCard syntax parsing rather than creating another protocol parser. Its upstream license is retained in nyx-core/crates/nyx-pim/data/ICAL-APACHE-2.0.txt and installed under this document directory's licenses/ subdirectory.

The bundled Unicode CLDR Windows time-zone mapping retains UNICODE-LICENSE.txt, also installed under licenses/. Other Rust dependencies retain their respective upstream licenses; the generated system bindings continue using the installed libical runtime.
