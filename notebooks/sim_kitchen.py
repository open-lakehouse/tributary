import marimo

__generated_with = "0.20.0"
app = marimo.App(width="medium")


@app.cell
def _():
    import marimo as mo
    import plotly.express as px
    import plotly.graph_objects as go
    import polars as pl

    return go, mo, pl, px


@app.cell
def _(mo):
    mo.md("""
    # Ghost Kitchen Simulation

    Configure and run the kitchen simulation, then explore the resulting events.
    """)
    return


@app.cell
def _(mo):
    steps_slider = mo.ui.slider(50, 2000, value=500, step=50, label="Steps")
    kitchens_slider = mo.ui.slider(1, 10, value=3, label="Kitchens")
    capacity_slider = mo.ui.slider(1, 20, value=5, label="Kitchen capacity")
    customers_slider = mo.ui.slider(5, 100, value=20, step=5, label="Customers")
    output_dir_input = mo.ui.text(
        value=".caspers/sim-kitchen",
        label="Output directory",
        full_width=True,
    )
    run_button = mo.ui.run_button(label="Run Simulation")

    mo.vstack([
        mo.hstack([steps_slider, kitchens_slider, capacity_slider, customers_slider]),
        output_dir_input,
        run_button,
    ])
    return (
        capacity_slider,
        customers_slider,
        kitchens_slider,
        output_dir_input,
        run_button,
        steps_slider,
    )


@app.cell
def _(
    capacity_slider,
    customers_slider,
    kitchens_slider,
    mo,
    output_dir_input,
    run_button,
    steps_slider,
):
    mo.stop(not run_button.value, mo.md("_Press **Run Simulation** to start._"))

    from caspers_universe import run_kitchen_simulation

    result = run_kitchen_simulation(
        steps=steps_slider.value,
        num_kitchens=kitchens_slider.value,
        kitchen_capacity=capacity_slider.value,
        num_customers=customers_slider.value,
        output_dir=output_dir_input.value,
    )

    mo.vstack([
        mo.md(f"""
        **Simulation complete**

        | Metric | Value |
        |--------|-------|
        | Steps | {result.total_steps} |
        | Agent ticks | {result.total_agent_ticks} |
        | Assignments | {result.total_assignments} |
        | Events path | `{result.events_path}` |
        """),
    ])
    return (result,)


@app.cell
def _(mo, pl, result):
    import os

    if not os.path.exists(result.events_path):
        mo.stop(True, mo.md("_No events file written — simulation produced 0 events._"))

    raw_events = pl.read_parquet(result.events_path)

    events = raw_events.with_columns(
        pl.col("time").str.to_datetime("%+").alias("timestamp"),
        pl.col("data").str.json_path_match(r"$.order_id").alias("order_id"),
        pl.col("data").str.json_path_match(r"$.customer_id").alias("customer_id"),
        pl.col("data").str.json_path_match(r"$.kitchen_id").alias("kitchen_id"),
    )

    mo.md(f"Loaded **{events.height}** events")
    return (events,)


@app.cell
def _(mo):
    mo.md("## Event Summary")
    return


@app.cell
def _(events, mo, pl):
    summary = (
        events.group_by("event_type")
        .agg(pl.len().alias("count"))
        .sort("count", descending=True)
    )

    time_min = events["timestamp"].min()
    time_max = events["timestamp"].max()

    mo.vstack([
        mo.md(
            f"**Time range**: {time_min} &rarr; {time_max}  \n"
            f"**Total events**: {events.height}  \n"
            f"**Distinct types**: {summary.height}"
        ),
        mo.ui.table(summary, label="Event counts by type"),
    ])
    return (summary,)


@app.cell
def _(px, summary):
    px.bar(
        summary.sort("count"),
        x="count",
        y="event_type",
        orientation="h",
        template="plotly_dark",
        title="Event Counts by Type",
    )
    return


@app.cell
def _(mo):
    mo.md("## Event Timeline")
    return


@app.cell
def _(events, pl, px):
    timeline_counts = (
        events.sort("timestamp")
        .group_by_dynamic("timestamp", every="30s", group_by="event_type")
        .agg(pl.len().alias("count"))
        .sort("timestamp")
    )

    px.scatter(
        timeline_counts,
        x="timestamp",
        y="count",
        color="event_type",
        template="plotly_dark",
        title="Events Over Time (30s buckets)",
        opacity=0.7,
    )
    return


@app.cell
def _(events, pl, px):
    cumulative = (
        events.select("timestamp", "event_type")
        .sort("timestamp")
        .with_columns(
            pl.lit(1)
            .cum_sum()
            .over("event_type")
            .alias("cumulative_count")
        )
    )
    px.line(
        cumulative,
        x="timestamp",
        y="cumulative_count",
        color="event_type",
        template="plotly_dark",
        title="Cumulative Events Over Time",
    )
    return


@app.cell
def _(mo):
    mo.md("""
    ## Workflow Traces

    Reconstruct order lifecycles by correlating events through `order_id`.
    Each order flows through: **created** &rarr; **assigned** &rarr; **completed** &rarr; **eating** &rarr; **done**.
    """)
    return


@app.cell
def _(events, mo, pl):
    order_events = events.filter(pl.col("order_id").is_not_null())

    if order_events.height == 0:
        workflow_traces = pl.DataFrame()
        mo.md("_No events with order_id found._")
    else:
        workflow_traces = (
            order_events.group_by("order_id")
            .agg(
                pl.col("event_type").alias("event_types"),
                pl.col("timestamp").min().alias("first_event"),
                pl.col("timestamp").max().alias("last_event"),
                pl.col("customer_id").first().alias("customer_id"),
                pl.len().alias("event_count"),
            )
            .sort("first_event")
        )
        mo.vstack([
            mo.md(f"Found **{workflow_traces.height}** distinct order workflows"),
            mo.ui.table(
                workflow_traces.select(
                    "order_id", "customer_id", "event_count",
                    "first_event", "last_event", "event_types",
                ),
                label="Order Workflow Traces",
            ),
        ])
    return (order_events,)


@app.cell
def _(go, mo, order_events, pl, px):
    if order_events.height == 0:
        mo.md("_No order events to build Gantt chart._")
    else:
        gantt_data = (
            order_events.group_by(["order_id", "event_type"])
            .agg(
                pl.col("timestamp").min().alias("start"),
                pl.col("timestamp").max().alias("end"),
            )
            .sort("start")
        )

        gantt_df = gantt_data.with_columns(
            pl.col("order_id").str.slice(-12).alias("short_id")
        )

        sample = gantt_df.head(200)

        fig = go.Figure()
        event_types = sample["event_type"].unique().sort().to_list()
        colors = px.colors.qualitative.Set2

        for i, et in enumerate(event_types):
            subset = sample.filter(pl.col("event_type") == et)
            durations = [
                max((e - s).total_seconds(), 1)
                for s, e in zip(subset["start"].to_list(), subset["end"].to_list())
            ]
            fig.add_trace(
                go.Bar(
                    y=subset["short_id"].to_list(),
                    x=durations,
                    base=[s.isoformat() for s in subset["start"].to_list()],
                    orientation="h",
                    name=et.replace("io.kitchen.", ""),
                    marker_color=colors[i % len(colors)],
                )
            )

        fig.update_layout(
            barmode="overlay",
            template="plotly_dark",
            title="Order Workflow Gantt (sample)",
            xaxis_title="Time",
            yaxis_title="Order (last 12 chars)",
            height=max(400, min(sample["short_id"].n_unique() * 30, 800)),
        )
        fig
    return


@app.cell
def _(mo):
    mo.md("## Duration Analysis")
    return


@app.cell
def _(events, mo, pl, px):
    order_created = (
        events.filter(pl.col("event_type") == "io.kitchen.order.created")
        .select("order_id", pl.col("timestamp").alias("created_at"))
    )

    order_assigned = (
        events.filter(pl.col("event_type") == "io.kitchen.order.assigned")
        .select("order_id", pl.col("timestamp").alias("assigned_at"))
    )

    order_completed = (
        events.filter(pl.col("event_type") == "io.kitchen.order.completed")
        .select("order_id", pl.col("timestamp").alias("completed_at"))
    )

    customer_eating = (
        events.filter(pl.col("event_type") == "io.kitchen.customer.eating")
        .select("order_id", pl.col("timestamp").alias("eating_at"))
    )

    durations = order_created
    for _df, _name in [
        (order_assigned, "assigned_at"),
        (order_completed, "completed_at"),
        (customer_eating, "eating_at"),
    ]:
        if _df.height > 0:
            durations = durations.join(_df, on="order_id", how="left")

    duration_cols = []
    if "assigned_at" in durations.columns:
        durations = durations.with_columns(
            (pl.col("assigned_at") - pl.col("created_at"))
            .dt.total_seconds()
            .alias("wait_time_s"),
        )
        duration_cols.append("wait_time_s")

    if "completed_at" in durations.columns and "assigned_at" in durations.columns:
        durations = durations.with_columns(
            (pl.col("completed_at") - pl.col("assigned_at"))
            .dt.total_seconds()
            .alias("prep_time_s"),
        )
        duration_cols.append("prep_time_s")

    if "completed_at" in durations.columns:
        durations = durations.with_columns(
            (pl.col("completed_at") - pl.col("created_at"))
            .dt.total_seconds()
            .alias("total_time_s"),
        )
        duration_cols.append("total_time_s")

    if "eating_at" in durations.columns:
        durations = durations.with_columns(
            (pl.col("eating_at") - pl.col("created_at"))
            .dt.total_seconds()
            .alias("order_to_eat_s"),
        )
        duration_cols.append("order_to_eat_s")

    if not duration_cols:
        mo.md(
            "_No duration data available. "
            "Run for more steps to see orders progress through "
            "created &rarr; assigned &rarr; completed._"
        )
    else:
        valid = durations.filter(
            pl.any_horizontal(pl.col(c).is_not_null() for c in duration_cols)
        )
        if valid.height == 0:
            mo.md("_All duration values are null — orders may not have progressed far enough._")
        else:
            duration_long = valid.unpivot(
                on=duration_cols,
                index="order_id",
                variable_name="metric",
                value_name="seconds",
            ).filter(pl.col("seconds").is_not_null())

            mo.vstack([
                mo.md(f"**{valid.height}** orders with timing data"),
                px.histogram(
                    duration_long,
                    x="seconds",
                    color="metric",
                    barmode="overlay",
                    template="plotly_dark",
                    title="Order Duration Distributions (seconds)",
                    opacity=0.7,
                ),
            ])
    return


@app.cell
def _(mo):
    mo.md("## Raw Event Browser")
    return


@app.cell
def _(events, mo, pl):
    type_filter = mo.ui.multiselect(
        options=sorted(events["event_type"].unique().to_list()),
        label="Filter by event type",
    )
    type_filter
    return (type_filter,)


@app.cell
def _(events, mo, pl, type_filter):
    filtered = (
        events.filter(pl.col("event_type").is_in(type_filter.value))
        if type_filter.value
        else events
    )
    mo.ui.table(
        filtered.select("timestamp", "event_type", "order_id", "customer_id", "kitchen_id", "data")
        .sort("timestamp"),
        label=f"{filtered.height} events",
    )
    return


if __name__ == "__main__":
    app.run()
