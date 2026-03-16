use std::sync::{Arc, LazyLock};

use arrow::{
    array::{ArrayRef, AsArray, FixedSizeBinaryBuilder, Int64Builder, RecordBatch},
    datatypes::{Float64Type, Int64Type},
};
use arrow_schema::DataType;
use datafusion::{
    common::JoinType,
    functions::core::expr_ext::FieldAccessor as _,
    functions_aggregate::expr_fn::{array_agg, first_value},
    functions_window::expr_fn::row_number,
    prelude::{DataFrame, Expr, cast, col, lit},
};
use fast_paths::{FastGraph, InputGraph, ShortestPath};
use futures::StreamExt as _;

use crate::{
    PersonId, Result, SimulationContext,
    functions::{h3_longlatash3, st_distance, st_length_spheroid, to_line_string},
};

pub struct MovementHandler {
    node_index: Vec<RecordBatch>,
    graph: FastGraph,
}

const NODE_INDEX_RESOLUTION: LazyLock<Expr> = LazyLock::new(|| lit(9_i8));

impl MovementHandler {
    pub(crate) async fn try_new(ctx: &SimulationContext) -> Result<Self> {
        let node_index = ctx
            .system()
            .routing_nodes()
            .await?
            .select([
                col("properties").field("osmid").alias("osmid"),
                h3_longlatash3()
                    .call(vec![
                        col("geometry").field("x"),
                        col("geometry").field("y"),
                        NODE_INDEX_RESOLUTION.clone(),
                    ])
                    .alias("node_index"),
                col("geometry"),
            ])?
            .sort(vec![col("osmid").sort(true, false)])?
            .select([
                cast(row_number() - lit(1_i64), DataType::Int64).alias("node_id"),
                col("osmid"),
                col("node_index"),
                col("geometry"),
            ])?
            .cache()
            .await?;

        let edges = ctx
            .system()
            .routing_edges()
            .await?
            .select([
                col("properties").field("osmid_source").alias("source"),
                col("properties").field("osmid_target").alias("target"),
                col("properties").field("length").alias("length"),
            ])?
            .join_on(
                node_index.clone(),
                JoinType::Right,
                [col("source").eq(col("osmid"))],
            )?
            .select([col("node_id").alias("source"), col("target"), col("length")])?
            .join_on(
                node_index.clone(),
                JoinType::Right,
                [col("target").eq(col("osmid"))],
            )?
            .select([col("source"), col("node_id").alias("target"), col("length")])?;

        let mut graph = InputGraph::new();
        let mut edge_stream = edges.execute_stream().await?;
        while let Some(batch) = edge_stream.next().await {
            process_batch(&mut graph, batch?)?;
        }
        graph.freeze();

        Ok(Self {
            node_index: node_index.collect().await?,
            graph: fast_paths::prepare(&graph),
        })
    }

    fn node_index(&self, ctx: &SimulationContext) -> Result<DataFrame> {
        Ok(ctx.ctx().read_batches(self.node_index.iter().cloned())?)
    }

    /// Plan journeys for persons moving through the world.
    ///
    /// The journey_data DataFrame should have the following columns:
    ///
    /// ```ignore
    /// {
    ///   person_id: bytes(16)
    ///   origin: {
    ///     x: float64
    ///     y: float64
    ///   }
    ///   destination: {
    ///     x: float64
    ///     y: float64
    ///   }
    /// }
    /// ```
    pub async fn plan_journeys(
        &mut self,
        ctx: &SimulationContext,
        journey_data: DataFrame,
    ) -> Result<Option<DataFrame>> {
        let journey_data = journey_data.cache().await?;

        // Check if any journeys need to be planned - if not, return early
        let journey_count = journey_data.clone().count().await?;
        if journey_count == 0 {
            return Ok(None);
        }

        let origin_ids = journey_data
            .clone()
            .select([
                col("person_id"),
                col("origin"),
                h3_longlatash3()
                    .call(vec![
                        col("origin").field("x"),
                        col("origin").field("y"),
                        NODE_INDEX_RESOLUTION.clone(),
                    ])
                    .alias("origin_index"),
            ])?
            .join_on(
                self.node_index(ctx)?.select([
                    col("node_index"),
                    col("node_id"),
                    col("geometry"),
                ])?,
                JoinType::Left,
                [col("origin_index").eq(col("node_index"))],
            )?
            .select([
                col("person_id"),
                col("node_id"),
                st_distance()
                    .call(vec![col("geometry"), col("origin")])
                    .alias("distance"),
            ])?
            .aggregate(
                vec![col("person_id")],
                vec![
                    first_value(col("node_id"), vec![col("distance").sort(true, false)])
                        .alias("node_id"),
                ],
            )?
            .select(vec![
                col("person_id").alias("person_id_origin"),
                col("node_id").alias("node_id_origin"),
            ])?;

        let destination_ids = journey_data
            .clone()
            .select([
                col("person_id"),
                col("destination"),
                h3_longlatash3()
                    .call(vec![
                        col("destination").field("x"),
                        col("destination").field("y"),
                        NODE_INDEX_RESOLUTION.clone(),
                    ])
                    .alias("destination_index"),
            ])?
            .join_on(
                self.node_index(ctx)?.select([
                    col("node_index"),
                    col("node_id"),
                    col("geometry"),
                ])?,
                JoinType::Left,
                [col("destination_index").eq(col("node_index"))],
            )?
            .select([
                col("person_id"),
                col("node_id"),
                st_distance()
                    .call(vec![col("geometry"), col("destination")])
                    .alias("distance"),
            ])?
            .aggregate(
                vec![col("person_id")],
                vec![
                    first_value(col("node_id"), vec![col("distance").sort(true, false)])
                        .alias("node_id"),
                ],
            )?
            .select(vec![
                col("person_id").alias("person_id_destination"),
                col("node_id").alias("node_id_destination"),
            ])?;

        let journey_data = journey_data
            .join_on(
                origin_ids,
                JoinType::Left,
                [col("person_id").eq(col("person_id_origin"))],
            )?
            .join_on(
                destination_ids,
                JoinType::Left,
                [col("person_id").eq(col("person_id_destination"))],
            )?
            .select_columns(&["person_id", "node_id_origin", "node_id_destination"])?;

        let mut calculator = fast_paths::create_calculator(&self.graph);
        let mut raw_journeys = vec![];

        let mut journey_stream = journey_data.execute_stream().await?;
        while let Some(Ok(batch)) = journey_stream.next().await {
            let mut raw_journey_builder = RawJourneyBuilder::new();

            let person_ids = batch
                .column(0)
                .as_fixed_size_binary()
                .iter()
                .flat_map(|maybe_id| maybe_id.and_then(|id| uuid::Uuid::from_slice(id).ok()))
                .map(PersonId::from)
                .collect::<Vec<_>>();

            let origin_indices = batch
                .column(1)
                .as_primitive::<Int64Type>()
                .iter()
                .collect::<Vec<_>>();

            let destination_indices = batch
                .column(2)
                .as_primitive::<Int64Type>()
                .iter()
                .collect::<Vec<_>>();

            let edge_iter = origin_indices
                .into_iter()
                .zip(destination_indices.into_iter());

            for (person_id, (origin_index, destination_index)) in
                person_ids.into_iter().zip(edge_iter)
            {
                if let (Some(origin_idx), Some(destination_idx)) = (origin_index, destination_index)
                {
                    let path = calculator.calc_path(
                        &self.graph,
                        origin_idx as usize,
                        destination_idx as usize,
                    );

                    if let Some(path) = &path {
                        raw_journey_builder.add_journey(&person_id, path)?;
                    } else {
                        println!("FAILED TO PLAN JOURNEY");
                    }
                }
            }

            raw_journeys.push(raw_journey_builder.finish()?);
        }

        if raw_journeys.is_empty() || raw_journeys.iter().all(|batch| batch.num_rows() == 0) {
            return Ok(None);
        }

        let df_journeys = ctx
            .ctx()
            .read_batches(raw_journeys)?
            .join_on(
                self.node_index(ctx)?
                    .select([col("node_id"), col("geometry")])?,
                JoinType::Left,
                [col("leg_node").eq(col("node_id"))],
            )?
            .select_columns(&["person_id", "leg_idx", "geometry"])?
            .sort(vec![
                col("person_id").sort(true, false),
                col("leg_idx").sort(true, false),
            ])?
            .aggregate(
                vec![col("person_id")],
                vec![array_agg(col("geometry")).alias("journey")],
            )?
            .with_column(
                "distance_m",
                st_length_spheroid().call(vec![to_line_string().call(vec![col("journey")])]),
            )?;

        Ok(Some(df_journeys))
    }
}

struct RawJourneyBuilder {
    person_id: FixedSizeBinaryBuilder,
    leg_idx: Int64Builder,
    leg_node: Int64Builder,
}

impl RawJourneyBuilder {
    fn new() -> Self {
        Self {
            person_id: FixedSizeBinaryBuilder::new(16),
            leg_idx: Int64Builder::new(),
            leg_node: Int64Builder::new(),
        }
    }

    fn add_journey(&mut self, person_id: &PersonId, path: &ShortestPath) -> Result<()> {
        for (idx, node_id) in path.get_nodes().iter().enumerate() {
            self.person_id
                .append_value(AsRef::<[u8]>::as_ref(person_id))?;
            self.leg_idx.append_value(idx as i64);
            self.leg_node.append_value(*node_id as i64);
        }

        Ok(())
    }

    fn finish(mut self) -> Result<RecordBatch> {
        let person_id_array: ArrayRef = Arc::new(self.person_id.finish());
        let leg_idx_array: ArrayRef = Arc::new(self.leg_idx.finish());
        let leg_node_array: ArrayRef = Arc::new(self.leg_node.finish());

        let batch = RecordBatch::try_from_iter(vec![
            ("person_id", person_id_array),
            ("leg_idx", leg_idx_array),
            ("leg_node", leg_node_array),
        ])?;

        Ok(batch)
    }
}

fn process_batch(graph: &mut InputGraph, batch: RecordBatch) -> Result<()> {
    let edge_iter = batch
        .column(0)
        .as_primitive::<Int64Type>()
        .iter()
        .zip(batch.column(1).as_primitive::<Int64Type>().iter())
        .zip(batch.column(2).as_primitive::<Float64Type>().iter());
    for ((source, target), length) in edge_iter {
        match (source, target, length) {
            (Some(source), Some(target), Some(length)) => {
                graph.add_edge(
                    source as usize,
                    target as usize,
                    length.round().abs() as usize,
                );
            }
            _ => {
                println!(
                    "Skipping edge with missing data: source={:?}, target={:?}, length={:?}",
                    source, target, length
                );
            }
        }
    }
    Ok(())
}
