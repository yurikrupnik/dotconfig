import .function_registry as fr
import .pipeline as pl

schema CrossplaneInput:
    oxr: {str:any}
    ocds?: [any]
    dxr?: {str:any}
    dcds?: [any]
    ctx?: {str:any}

schema CrossplaneFunctionConfig:
    pipeline: pl.Pipeline
    oxr: {str:any}

execute_as_crossplane_function = lambda config: CrossplaneFunctionConfig -> {str:any} {
    _resources = pl.execute_pipeline_steps(config.pipeline)

    {
        apiVersion: "krm.kcl.dev/v1alpha1"
        kind: "KCLRun"
        metadata.name: "crossplane-function-output"
        spec.resources: _resources
    }
}

create_crossplane_function = lambda steps: [any], oxr: any -> any {
    _pipeline = pl.create_pipeline([pl.PipelineStep {functionName: s.functionName, input: s.input} for s in steps], None)

    execute_as_crossplane_function(CrossplaneFunctionConfig {
        pipeline: _pipeline
        oxr: oxr
    })
}

create_dynamic_composition = lambda functionName: str, inputData: any -> {str:any} {
    {
        apiVersion: "apiextensions.crossplane.io/v1"
        kind: "Composition"
        metadata.name: "dynamic-${functionName}"
        spec: {
            compositeTypeRef: {
                apiVersion: "yurikrupnik.com/v1alpha1"
                kind: "XDynamicResource"
            }
            mode: "Pipeline"
            pipeline: [
                {
                    step: "execute-${functionName}"
                    functionRef.name: "function-kcl"
                    input: {
                        apiVersion: "krm.kcl.dev/v1alpha1"
                        kind: "KCLInput"
                        spec: {
                            target: "Resources"
                            source: """
import function_registry as fr

output = fr.execute_function(fr.registry, "${functionName}", ${inputData})
items = output if typeof(output) == "list" else [output]
"""
                        }
                    }
                }
                {
                    step: "automatically-detect-ready"
                    functionRef.name: "function-auto-ready"
                }
            ]
        }
    }
}
