FROM ubuntu:22.04 as base
ARG TARGETPLATFORM
COPY . /tmp
WORKDIR /tmp

RUN echo $TARGETPLATFORM
RUN ls -R /tmp/
# move the binary to root based on platform
RUN case $TARGETPLATFORM in \
        "linux/amd64")  BUILD=x86_64-unknown-linux-gnu  ;; \
        "linux/arm64")  BUILD=aarch64-unknown-linux-gnu  ;; \
        *) exit 1 ;; \
    esac; \
    mv /tmp/$BUILD/atm0s-media-server-$BUILD /atm0s-media-server; \
    mv /tmp/$BUILD/convert_record_cli-$BUILD /convert_record_cli; \
    mv /tmp/$BUILD/convert_record_worker-$BUILD /convert_record_worker; \
    chmod +x /atm0s-media-server; \
    chmod +x /convert_record_cli; \
    chmod +x /convert_record_worker

FROM ubuntu:22.04

# install wget & curl
RUN apt update && apt install -y wget curl && apt clean && rm -rf /var/lib/apt/lists/*

COPY maxminddb-data /maxminddb-data
COPY --from=base /atm0s-media-server /atm0s-media-server
COPY --from=base /convert_record_cli /convert_record_cli
COPY --from=base /convert_record_worker /convert_record_worker
ENTRYPOINT ["/atm0s-media-server"]
