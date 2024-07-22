DOCKER_TAG=$IMAGE:$VERSION
docker build -t $DOCKER_TAG -f Dockerfile .
if [[ -n $DOCKER_PUSH && $DOCKER_PUSH == "true" ]]; then 
docker push $DOCKER_TAG
fi

