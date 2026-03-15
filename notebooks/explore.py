import marimo

__generated_with = "0.18.4"
app = marimo.App(width="medium")


@app.cell
def _():
    import os

    import folium
    import marimo as mo
    import plotly.express as px
    import polars as pl
    return folium, mo, os, pl, px


@app.cell
def _(os):
    # create data directories
    os.makedirs("./data/objects", exist_ok=True)
    os.makedirs("./data/people", exist_ok=True)
    return


@app.cell
def _(os):
    # delete all files in data directories
    for file in os.listdir("./data/objects"):
        os.remove(os.path.join("./data/objects", file))
    for file in os.listdir("./data/people"):
        os.remove(os.path.join("./data/people", file))
    return


@app.cell
def _(mo):
    _orders_counts = mo.sql(
        f"""
        SELECT status, count(*) as count
        FROM './data/orders/*.parquet'
        GROUP BY status
        """
    )
    return


@app.cell
def _(mo):
    _order_lines_counts = mo.sql(
        f"""
        SELECT status, count(*) as count
        FROM './data/order_lines/*.parquet'
        GROUP BY status
        """
    )
    return


@app.cell
def _(mo):
    _object_counts = mo.sql(
        f"""
        SELECT label, count(*) as count
        FROM './data//objects/*.parquet'
        GROUP BY label
        """
    )
    return


@app.cell
def _(mo):
    _people_counts = mo.sql(
        f"""
        SELECT role, count(*) as count
        FROM './data/population/people/*.parquet'
        GROUP BY role
        """
    )
    return


@app.cell
def _(mo):
    _df = mo.sql(
        f"""
        SELECT type, count(*)
        FROM './data/events/*.json'
        GROUP BY type
        """
    )
    return


@app.cell
def _(mo):
    trips = mo.sql(
        f"""
        SELECT
            -- workaround to not panic during output
            people.id::VARCHAR as id,
            MIN(pos.timestamp) as start_time,
            MAX(pos.timestamp) as end_time,
            ARRAY_AGG(
                pos.timestamp
                ORDER BY
                    pos.timestamp
            ) as timestamps,
            ARRAY_AGG(
                pos.position
                ORDER BY
                    pos.timestamp
            ) as positions
        FROM
            './data/population/people/*.parquet' as people
            LEFT JOIN (
                SELECT
                    *,
                    position.x - lag (position.x) OVER (
                        PARTITION BY
                            id
                        ORDER BY
                            timestamp
                    ) as diff
                FROM
                    './data/population/positions/*.parquet'
            ) as pos ON people.id = pos.id
        WHERE
            role == 'courier'
            AND abs(diff) > 1e-10
        GROUP BY
            people.id
        LIMIT
            10
        """
    )
    return (trips,)


@app.cell
def _():
    import datetime
    from typing import Any, Iterable, TypedDict

    class TripPoint(TypedDict):
        x: float
        y: float

    class TripData(TypedDict):
        timestamps: list[datetime.datetime]
        positions: list[TripPoint]

    def trip_geo_features(trips: Iterable[TripData]) -> list[dict[str, Any]]:
        """Generate GeoJson Features from trip data."""

        return [
            {
                "type": "Feature",
                "geometry": {
                    "type": "LineString",
                    "coordinates": [[p["x"], p["y"]] for p in trip["positions"]],
                },
                "properties": {
                    "times": [ts.isoformat() for ts in trip["timestamps"]],
                    "style": {"color": "blue", "weight": 3, "opacity": 0.6},
                    "icon": "circle",
                    "iconstyle": {
                        "fillColor": "red",
                        "fillOpacity": 0.6,
                        "stroke": "false",
                        "radius": 5,
                    },
                },
            }
            for trip in trips
        ]
    return (trip_geo_features,)


@app.cell
def _(folium, trip_geo_features, trips):
    from folium.plugins import TimestampedGeoJson

    lng, lat = -0.13381370382489707, 51.518898098201326
    features = trip_geo_features(trips.to_dicts())

    # Create a map centered on the first person's location
    m = folium.Map(
        location=[lat, lng],
        zoom_start=13,
        tiles="CartoDB dark_matter",  # Dark theme map
    )

    TimestampedGeoJson(
        {"type": "FeatureCollection", "features": features},
        add_last_point=True,
        loop=True,
        auto_play=False,
        date_options="YYYY-MM-DD HH:mm:ss",
        period="PT1M",  # 5 minutes between points
        duration="PT1M",  # 1 minute display duration for each point
        loop_button=True,
    ).add_to(m)

    m
    return


@app.cell
def _(pl, px):
    df = pl.scan_parquet("./data/stats/orders/").collect()
    px.line(
        df,
        x="timestamp",
        y="count",
        color="status",
        template="plotly_dark",
        title="Order Status",
    )
    return


@app.cell
def _(pl, px):
    df_events = pl.scan_parquet("./data/stats/events/").collect()
    px.line(
        df_events,
        x="timestamp",
        y="value",
        color="label",
        template="plotly_dark",
        title="Events Raised",
    )
    return


@app.cell
def _(px):
    import math

    sq_sigma = 0.4

    def bell(x: float, mu: float) -> float:
        return (1 / math.sqrt(2 * math.pi * sq_sigma)) * math.e ** (
            -1 * (x - mu) ** 2 / (2 * sq_sigma)
        )

    x_values = [0 + 24 * idx / 1000 for idx in range(1000)]
    y_values = [bell(v, 12) + bell(v, 18) for v in x_values]

    px.line(
        x=x_values,
        y=y_values,
        template="plotly_dark",
    )
    return


@app.cell
def _(mo):
    _df = mo.sql(
        f"""
        SELECT *
        FROM '../.caspers/snapshots/objects/*.parquet'
        LIMIT 3
        """
    )
    return


@app.cell
def _():
    import delta
    import delta.pip_utils
    import delta.tables
    import pyspark
    from pyspark.sql.types import StructType, StructField, StringType, IntegerType


    def get_spark():
        builder = (
            pyspark.sql.SparkSession.builder.appName("MyApp")
            .config("spark.sql.extensions", "io.delta.sql.DeltaSparkSessionExtension")
            .config(
                "spark.sql.catalog.spark_catalog",
                "org.apache.spark.sql.delta.catalog.DeltaCatalog",
            )
            .config("spark.sql.ansi.enabled", "false")
        )
        return delta.pip_utils.configure_spark_with_delta_pip(builder).getOrCreate()

    schema = StructType([
        StructField("id", IntegerType(), False),
        StructField("name", StringType(), True),
        StructField("age", IntegerType(), True),
        StructField("email", StringType(), True)
    ])

    # Create sample data
    data = [
        (1, "Alice", 30, "alice@example.com"),
        (2, "Bob", 25, "bob@example.com"),
        (3, "Charlie", 35, "charlie@example.com"),
        (4, "Diana", 28, "diana@example.com")
    ]

    spark = get_spark()

    # Create DataFrame
    df = spark.createDataFrame(data, schema)

    # Path for the Delta table
    table_path = "./tmp/column_mapping"

    # Write DataFrame as Delta table with column mapping enabled
    df.write \
        .format("delta") \
        .mode("overwrite") \
        .option("delta.columnMapping.mode", "name") \
        .option("overwriteSchema", "true") \
        .save(table_path)
    return


if __name__ == "__main__":
    app.run()
