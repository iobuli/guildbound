shader_type spatial;
render_mode unshaded;

group_uniforms Color;
uniform vec3 primary_color : source_color = vec3(1.0,0.8,0.2);
uniform vec3 secondary_color : source_color = vec3(1.0,0.4,0.1);
uniform vec3 tertiary_color : source_color = vec3(0.6,0.1,0.05);
uniform float emission = 2.0;
uniform float color_curve = 1.0;
uniform float hue_variation = 0.1;

group_uniforms Transparency;
uniform float edge_hardness : hint_range(0.0, 1.0, 0.01) = 0.0;
uniform float edge_position : hint_range(0.0, 1.0, 0.01) = 0.2;
uniform bool proximity_fade = false;
uniform float proximity_fade_distance : hint_range(0.0, 4096.0, 0.01);
uniform sampler2D depth_texture : hint_depth_texture, repeat_disable, filter_nearest;

varying flat int id;

vec3 shift_hue(vec3 color, float shift) {
	vec3 gray = vec3(0.57735);
	vec3 projection = gray * dot(gray, color.rgb);
	vec3 U = color.rgb - projection;
	vec3 V = cross(gray, U);
	vec3 shifted = U * cos(shift * 2.0 * PI) + V * sin(shift * 2.0 * PI) + projection;
	return shifted;
}

void vertex() {
	id = INSTANCE_ID;
}

void fragment() {
	vec2 centered_uv = UV * 2.0 - 1.0;
	float r = length(centered_uv);
	float angle = atan(centered_uv.x, centered_uv.y);
	
	float rand1 = floor(COLOR.r * 5.0);
	float rand2 = float(id % 3);
	
	float circle_mask = max(1.0 - r, 0.0);
	float branches = pow(sin((r) * PI * (3.0 + rand1)) * 0.5 + 0.5, 10.0) * circle_mask;
	float streaks = cos(angle* (5.0 + rand2)) + pow(max(1.0 - r, 0.0), 4.0) * 4.0;
	streaks += branches * 3.0;
	streaks = step(1.0, streaks * COLOR.a);
	
	ALPHA = streaks;
	
	vec3 color = mix(secondary_color, primary_color, 1.0 - UV.y);
	color = shift_hue(color, (COLOR.r - 0.5) * hue_variation);
	
	ALBEDO = color * emission;
	
	
	// Proximity Fade: Enabled
	if(proximity_fade){
		float proximity_depth_tex = textureLod(depth_texture, SCREEN_UV, 0.0).r;
		vec4 ndc = OUTPUT_IS_SRGB ? vec4(vec3(SCREEN_UV, proximity_depth_tex) * 2.0 - 1.0, 1.0) : vec4(SCREEN_UV * 2.0 - 1.0, proximity_depth_tex, 1.0);
		vec4 proximity_view_pos = INV_PROJECTION_MATRIX * ndc;
		proximity_view_pos.xyz /= proximity_view_pos.w;
		ALPHA *= clamp(1.0 - smoothstep(proximity_view_pos.z + proximity_fade_distance, proximity_view_pos.z, VERTEX.z), 0.0, 1.0);
	}
	
	ALPHA = clamp((ALPHA - edge_position) / (1.0 - edge_hardness), 0.0, 1.0);
}