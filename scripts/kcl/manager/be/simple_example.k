import .function_registry as fr
import .pipeline as pl

simple_namespace_example = lambda {
    [fr.execute_function(fr.registry, "create_namespace", {
        name: "simple-example"
        labels: {"app": "demo", "env": "test"}
    })]
}

simple_pipeline_example = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {
                name: "demo-app"
                labels: {"app": "demo", "managed-by": "kcl"}
            }
        }
        {
            functionName: "create_secret"
            input: {
                name: "demo-secret"
                namespace: "demo-app"
                data: {"username": "ZGVtby11c2Vy"}
                labels: {"app": "demo"}
            }
        }
    ]
    pl.execute_pipeline(_steps, None)
}

items = simple_pipeline_example()
