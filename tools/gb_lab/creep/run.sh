# $1 option name, $2 variant name, rest: args
o=$1; v=$2; shift 2
target/release/gb_lab stand --scaled --no_clamp --balance --substeps 12 "$@" > creep/${o}__${v}.json
