import .function_registry as fr

schema PipelineStep:
    functionName: str
    input: any

schema Pipeline:
    steps: [PipelineStep]
    registry: fr.FunctionRegistry

_execute_step = lambda registry: fr.FunctionRegistry, step: PipelineStep -> [any] {
    _result = fr.execute_function(registry, step.functionName, step.input)
    _result if typeof(_result) == "list" else [_result]
}

_execute_recursive = lambda registry: fr.FunctionRegistry, steps: [PipelineStep], index: int -> [any] {
    _currentResult = [] if index >= len(steps) else _execute_step(registry, steps[index])
    _remainingResult = [] if index >= len(steps) else _execute_recursive(registry, steps, index + 1)
    _currentResult + _remainingResult
}

execute_pipeline_steps = lambda pipeline: Pipeline -> [any] {
    _execute_recursive(pipeline.registry, pipeline.steps, 0)
}

create_pipeline = lambda steps: [PipelineStep], registry: any -> Pipeline {
    _reg = registry if typeof(registry) == "FunctionRegistry" else fr.registry
    Pipeline {
        steps: steps
        registry: _reg
    }
}

execute_pipeline = lambda steps: [any], registry: any -> [any] {
    _pipelineSteps = [PipelineStep {functionName: s.functionName, input: s.input} for s in steps]
    _reg = registry if typeof(registry) == "FunctionRegistry" else fr.registry
    _pipeline = create_pipeline(_pipelineSteps, _reg)
    execute_pipeline_steps(_pipeline)
}
