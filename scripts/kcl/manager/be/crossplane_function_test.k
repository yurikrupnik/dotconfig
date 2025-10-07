import .crossplane_function as cf
import .pipeline as pl
import .function_registry as fr

test_create_crossplane_function = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {name: "crossplane-ns"}
        }
    ]

    _oxr = {
        spec: {
            parameters: {
                name: "test-resource"
            }
        }
    }

    _output = cf.create_crossplane_function(_steps, _oxr)

    assert _output.apiVersion == "krm.kcl.dev/v1alpha1"
    assert _output.kind == "KCLRun"
    assert _output.metadata.name == "crossplane-function-output"
    assert len(_output.spec.resources) == 1
}

test_execute_as_crossplane_function = lambda {
    _steps = [
        pl.PipelineStep {
            functionName: "create_secret"
            input: {
                name: "crossplane-secret"
                namespace: "default"
            }
        }
    ]

    _pipeline = pl.create_pipeline(_steps, None)

    _config = cf.CrossplaneFunctionConfig {
        pipeline: _pipeline
        oxr: {spec: {parameters: {}}}
    }

    _output = cf.execute_as_crossplane_function(_config)

    assert _output.apiVersion == "krm.kcl.dev/v1alpha1"
    assert _output.kind == "KCLRun"
    assert len(_output.spec.resources) == 1
}

test_create_dynamic_composition = lambda {
    _input = {
        name: "test-db"
        namespace: "apps"
    }

    _composition = cf.create_dynamic_composition("create_secret", _input)

    assert _composition.apiVersion == "apiextensions.crossplane.io/v1"
    assert _composition.kind == "Composition"
    assert _composition.metadata.name == "dynamic-create_secret"
    assert _composition.spec.mode == "Pipeline"
    assert len(_composition.spec.pipeline) == 2
    assert _composition.spec.pipeline[0].functionRef.name == "function-kcl"
    assert _composition.spec.pipeline[1].functionRef.name == "function-auto-ready"
}

test_crossplane_function_with_multiple_steps = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {name: "multi-ns"}
        }
        {
            functionName: "create_secret"
            input: {
                name: "multi-secret"
                namespace: "multi-ns"
            }
        }
        {
            functionName: "create_cluster_secret_store"
            input: {
                name: "multi-store"
                projectID: "test-project"
                secretName: "creds"
                secretKey: "key"
            }
        }
    ]

    _oxr = {spec: {parameters: {region: "us-east-1"}}}

    _output = cf.create_crossplane_function(_steps, _oxr)

    assert len(_output.spec.resources) == 3
}

test_create_crossplane_function()
test_execute_as_crossplane_function()
test_create_dynamic_composition()
test_crossplane_function_with_multiple_steps()
