use geodatafusion::udf::geo::measurement::Distance;

use crate::functions::length_spheroid::LengthSpheroid;

mod h3_long_lat_as_h3;
mod interpolate_line;
mod length_spheroid;
mod to_line_string;
mod uuid_to_str;
mod uuid_v7;

#[macro_export]
macro_rules! make_udf_function {
    ($UDF:ty, $NAME:ident) => {
        #[allow(rustdoc::redundant_explicit_links)]
        #[doc = concat!("Return a [`ScalarUDF`](datafusion_expr::ScalarUDF) implementation of ", stringify!($NAME))]
        pub fn $NAME() -> std::sync::Arc<datafusion::logical_expr::ScalarUDF> {
            // Singleton instance of the function
            static INSTANCE: std::sync::LazyLock<
                std::sync::Arc<datafusion::logical_expr::ScalarUDF>,
            > = std::sync::LazyLock::new(|| {
                std::sync::Arc::new(datafusion::logical_expr::ScalarUDF::new_from_impl(
                    <$UDF>::new(),
                ))
            });
            std::sync::Arc::clone(&INSTANCE)
        }
    };
}

make_udf_function!(h3_long_lat_as_h3::LongLatAsH3, h3_longlatash3);
make_udf_function!(uuid_v7::UuidV7, uuidv7);
make_udf_function!(uuid_to_str::UuidToString, uuid_to_string);
make_udf_function!(LengthSpheroid, st_length_spheroid);
make_udf_function!(Distance, st_distance);
make_udf_function!(interpolate_line::InterpolateLine, st_line_interpolate_point);
make_udf_function!(to_line_string::ToLineString, to_line_string);
