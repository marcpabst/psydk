Running experiments
====================

Anatomy of an Experiment
----------------------

Experiments are simple Python functions that take a :class:`~psydk.ExperimentContext` object as the first argument and a any number of additional arguments. They can be optionally decorated with the :func:`~psydk.experiment` decorator to automatically handle setting up the experiment context.

The :class:`~psydk.ExperimentContext` object provides methods for controlling the experiment, such as creating windows and starting audio streams.

.. automodule:: psydk
   :members:
   :undoc-members:

Controling the Timing
---------------------

.. automodule:: psydk.time
   :members:
   :undoc-members:
