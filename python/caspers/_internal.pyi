class KitchenResult:
    @property
    def events_path(self) -> str:
        """Absolute path to the written events.parquet file."""

    @property
    def total_steps(self) -> int:
        """Number of simulation steps executed."""

    @property
    def total_agent_ticks(self) -> int:
        """Total number of agent tick calls across all steps."""

    @property
    def total_assignments(self) -> int:
        """Total number of task-to-resource assignments made."""

class LogisticsResult:
    @property
    def events_path(self) -> str:
        """Absolute path to the written events.parquet file."""

    @property
    def packages_created(self) -> int:
        """Total packages created during the simulation."""

    @property
    def packages_delivered(self) -> int:
        """Total packages delivered during the simulation."""

    @property
    def driver_utilization_percent(self) -> float:
        """Average driver utilization as a percentage (0-100)."""

    @property
    def average_delivery_time_secs(self) -> float:
        """Average end-to-end delivery time in seconds."""

def run_kitchen_simulation(
    steps: int = 100,
    num_kitchens: int = 3,
    kitchen_capacity: int = 5,
    num_customers: int = 20,
    output_dir: str = ".caspers/sim-kitchen",
    seed: int | None = None,
) -> KitchenResult:
    """Run the ghost kitchen simulation and write events to Parquet.

    Args:
        steps: Number of simulation steps to run.
        num_kitchens: Number of kitchens.
        kitchen_capacity: Concurrent order capacity per kitchen.
        num_customers: Number of customer agents.
        output_dir: Directory to write events.parquet (and artifacts/) into.
        seed: Global seed. A seeded run starts at 2026-01-05T11:00:00Z and
            reproduces its events (all but the CloudEvents ids) and artifacts.

    Returns:
        KitchenResult with events_path and run statistics.
    """

def run_logistics_simulation(
    steps: int = 120,
    output_dir: str = ".caspers/sim-logistics",
    seed: int | None = None,
) -> LogisticsResult:
    """Run the logistics (package delivery) simulation and write events to Parquet.

    Args:
        steps: Number of simulation steps (30 seconds each; 120 steps = 1 hour).
        output_dir: Directory to write events.parquet into.
        seed: Global seed. A seeded run starts at 2026-01-05T11:00:00Z and
            reproduces its events (all but the CloudEvents ids).

    Returns:
        LogisticsResult with events_path and delivery metrics.
    """
