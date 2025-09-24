docker_build(
  "yurikrupnik/dotconfig",
  ".",
  dockerfile="./rust-cli.Dockerfile",
  #build_args={"APP_NAME":"playground_api"},
)

k8s_yaml([
    "k8s-manifests/app.yaml"
])

#text = local('./src/main.rs') # runs command foo.py
#k8s_yaml(text)

#k8s_resource("actix-app", port_forwards="5201:8080")
#k8s_resource(
#  workload='frontend',
#  objects=['frontend:secret', 'frontend:volume']
#)
# Load the 'deployment' extension
# load('ext://deployment', 'deployment_create')
# Create a redis deployment and service with a readiness probe
#deployment_create(
#  'redis',
#  ports='6379',
#  readiness_probe={'exec':{'command':['redis-cli','ping']}}
#)

# docker_compose('/Users/yurikrupnik/projects/playground/manifests/dockers/compose.yaml')

#k8s_resource("my-resource", auto_init=False, trigger_mode=TRIGGER_MODE_MANUAL)

#local_resource("my-resource", serve_cmd="./run.sh", auto_init=False, trigger_mode=TRIGGER_MODE_MANUAL)

#dc_resource("my-resouce", auto_init=False, trigger_mode=TRIGGER_MODE_MANUAL)