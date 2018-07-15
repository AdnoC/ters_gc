extern crate proc_macro;
#[macro_use]
extern crate synstructure;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

fn trace_derive(mut s: synstructure::Structure) -> TokenStream {
    // No way to check whether a field implements a trait, so have an attribute
    // to ignore fields that don't implement Trace.
    // https://github.com/dtolnay/syn/issues/77
    s.filter(|bind_info| {
        !bind_info
            .ast()
            .attrs
            .iter()
            .any(|attr| match attr.interpret_meta() {
                Some(meta) => meta.name() == "ignore_trace",
                None => false,
            })
    });

    let body = s.each(|bind_info| {
        quote! {
            _tracer.add_target(#bind_info);
        }
    });

    s.gen_impl(quote! {
        extern crate ters_gc;
        gen impl ters_gc::trace::Trace for @Self {
            fn trace(&self, _tracer: &mut ters_gc::trace::Tracer) {
                match *self {
                    #body
                }
            }
        }
    }).into()
}

decl_derive!([Trace, attributes(ignore_trace)] => trace_derive);
