import .pipeline as pl
import .function_registry as fr

test_create_pipeline = lambda {
    _steps = [
        pl.PipelineStep {
            functionName: "create_namespace"
            input: {name: "pipeline-ns"}
        }
        pl.PipelineStep {
            functionName: "create_secret"
            input: {
                name: "pipeline-secret"
                namespace: "pipeline-ns"
            }
        }
    ]

    _pipeline = pl.create_pipeline(_steps, None)

    assert len(_pipeline.steps) == 2
    assert _pipeline.steps[0].functionName == "create_namespace"
}

test_pipeline_execute = lambda {
    _steps = [
        pl.PipelineStep {
            functionName: "create_namespace"
            input: {name: "exec-ns", labels: {"test": "true"}}
        }
        pl.PipelineStep {
            functionName: "create_secret"
            input: {
                name: "exec-secret"
                namespace: "exec-ns"
                data: {"foo": "bar"}
            }
        }
    ]

    _pipeline = pl.create_pipeline(_steps, None)
    _results = pl.execute_pipeline_steps(_pipeline)

    assert len(_results) == 2
    assert _results[0].metadata.name == "exec-ns"
    assert _results[1].metadata.name == "exec-secret"
}

test_execute_pipeline_helper = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {name: "helper-ns"}
        }
        {
            functionName: "create_cluster_secret_store"
            input: {
                name: "helper-store"
                projectID: "test-project"
                secretName: "creds"
                secretKey: "key"
            }
        }
    ]

    _results = pl.execute_pipeline(_steps, None)

    assert len(_results) == 2
    assert _results[0].metadata.name == "helper-ns"
    assert _results[1].metadata.name == "helper-store"
}

test_pipeline_with_multiple_resources = lambda {
    _steps = [
        pl.PipelineStep {
            functionName: "create_namespace"
            input: {name: "multi-ns-1"}
        }
        pl.PipelineStep {
            functionName: "create_namespace"
            input: {name: "multi-ns-2"}
        }
        pl.PipelineStep {
            functionName: "create_namespace"
            input: {name: "multi-ns-3"}
        }
    ]

    _pipeline = pl.create_pipeline(_steps, None)
    _results = pl.execute_pipeline_steps(_pipeline)

    assert len(_results) == 3
    assert _results[0].metadata.name == "multi-ns-1"
    assert _results[1].metadata.name == "multi-ns-2"
    assert _results[2].metadata.name == "multi-ns-3"
}

test_pipeline_with_custom_registry = lambda {
    _custom_fn = lambda input: any -> {str:any} {
        {custom: input.value}
    }

    _custom_registry = fr.register_function(fr.registry, "custom_function", _custom_fn)

    _steps = [
        pl.PipelineStep {
            functionName: "custom_function"
            input: {value: "test"}
        }
    ]

    _pipeline = pl.create_pipeline(_steps, _custom_registry)
    _results = pl.execute_pipeline_steps(_pipeline)

    assert len(_results) == 1
    assert _results[0].custom == "test"
}

test_create_pipeline()
test_pipeline_execute()
test_execute_pipeline_helper()
test_pipeline_with_multiple_resources()
test_pipeline_with_custom_registry()
