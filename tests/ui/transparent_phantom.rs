use std::marker::PhantomData;

use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(transparent)]
struct Marker(PhantomData<u8>);

fn main() {}
