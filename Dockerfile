FROM debian:buster-slim AS uni
ENV dep_version=1-6-0
ENV dep_dir_version=1.6.0
ENV unimrcp_version=1-8-0
ENV dir_version=1.8.0

WORKDIR /root
# wget -- to load sources of unimrcp-deps and unimrcp
# autoconf, automake, libtool, gcc, g++, pkg-config, sudo, make -- to build from sources
# openssl-dev -- uniMRCP dependency
# clang -- build.rs dependency
RUN apt-get update && apt-get install -y wget autoconf automake libtool gcc g++ pkg-config sudo make clang libssl-dev && apt-get clean
RUN wget --no-check-certificate -O unimrcp-deps-${dep_version}-tar.gz http://www.unimrcp.org/project/component-view/unimrcp-deps-${dep_version}-tar-gz/download && \
    tar -xzvf unimrcp-deps-${dep_version}-tar.gz && rm unimrcp-deps-${dep_version}-tar.gz && \
    cd unimrcp-deps-${dep_dir_version}  && ./build-dep-libs.sh -s
RUN wget --no-check-certificate -O unimrcp-${unimrcp_version}-tar.gz http://www.unimrcp.org/project/component-view/unimrcp-${unimrcp_version}-tar-gz/download && \
    tar -xzvf unimrcp-${unimrcp_version}-tar.gz && rm unimrcp-${unimrcp_version}-tar.gz && \
    cd unimrcp-${dir_version} && ./bootstrap && ./configure && make && make install && ldconfig

FROM rust:1.72-buster AS build
RUN apt-get update && apt-get install -y clang libssl-dev && apt-get clean

RUN mkdir -p /usr/local/unimrcp
RUN mkdir -p /usr/local/apr
COPY --from=uni /usr/local/unimrcp /usr/local/unimrcp
COPY --from=uni /usr/local/apr /usr/local/apr
COPY --from=uni /usr/local/lib /usr/local/lib
RUN ldconfig
ENV UNIMRCP_PATH="/usr/local/unimrcp"
ENV APR_LIB_PATH="/usr/local/apr"
ENV APR_INCLUDE_PATH="/usr/local/apr"
WORKDIR /root/demo_asr
COPY . .
RUN cargo build --release

FROM debian:buster-slim
LABEL maintainer="Alexander Kalashnikov"

RUN apt-get update && apt-get install -y libssl-dev && apt-get clean

RUN mkdir -p /usr/local/unimrcp
RUN mkdir -p /usr/local/apr
COPY --from=uni /usr/local/unimrcp /usr/local/unimrcp
COPY --from=uni /usr/local/apr /usr/local/apr
COPY --from=uni /usr/local/lib /usr/local/lib
RUN ldconfig

COPY --from=build /root/demo_asr/target/release/librs_unimrcp_recog.so /usr/local/unimrcp/plugin/librs_unimrcp_recog.so
COPY conf/unimrcpserver.xml /usr/local/unimrcp/conf/unimrcpserver.xml

ENV RUST_LOG="rs_unimrcp_recog=trace"
WORKDIR /usr/local/unimrcp/bin
CMD [ "./unimrcpserver", "-r", "/usr/local/unimrcp", "-o", "1", "-l", "7" ]
