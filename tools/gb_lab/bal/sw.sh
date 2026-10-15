G=""; [ "$5" = ge ] && G="--geom_edge"
target/release/gb_lab stand --scaled --no_clamp --balance --rest_offset_off --kb $1 --kbd $2 --kp $3 --kd $4 $G > bal/sw/$1_$2_$3_$5.json
