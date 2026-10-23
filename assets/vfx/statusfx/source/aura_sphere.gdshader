shader_type spatial;
render_mode unshaded, cull_disabled, blend_add;

group_uniforms Color;
uniform vec3 primary_color : source_color = vec3(1.0,0.8,0.2);
uniform vec3 secondary_color : source_color = vec3(1.0,0.4,0.1);
uniform vec3 tertiary_color : source_color = vec3(0.6,0.1,0.05);
uniform float emission = 2.0;
uniform float hue_variation = 0.0;
uniform float color_curve = 1.0;

group_uniforms Shape;
uniform sampler2D noise_texture;
uniform vec2 noise_scale = vec2(3.0, 1.0);
uniform vec2 noise_scroll = vec2(0.1, 0.4);
uniform float noise_twist : hint_range(-1.0, 1.0, 0.05) = 1.0;
uniform float shape_curve = 2.0;
uniform float waviness : hint_range(0.0, 1.0, 0.01) = 0.2;

group_uniforms Transparency;
uniform float edge_hardness : hint_range(0.0, 1.0, 0.01) = 0.0;
uniform float edge_position : hint_range(0.0, 1.0, 0.01) = 0.2;
uniform bool proximity_fade = false;
uniform float proximity_fade_distance : hint_range(0.0, 4096.0, 0.01);
uniform sampler2D depth_texture : hint_depth_texture, repeat_disable, filter_nearest;

vec3 shift_hue(vec3 color, float shift) {
	vec3 gray = vec3(0.57735);
	vec3 projection = gray * dot(gray, color.rgb);
	vec3 U = color.rgb - projection;
	vec3 V = cross(gray, U);
	vec3 shifted = U * cos(shift * 2.0 * PI) + V * sin(shift * 2.0 * PI) + projection;
	return shifted;
}

float sample_value(vec2 uv){
	vec2 noise_uv = vec2(uv.x - uv.y * noise_twist, uv.y) * noise_scale + (TIME * noise_scroll);
	float noise = texture(noise_texture, noise_uv).r;
	
	float mask = pow((1.0 - abs(uv.y * 2.0 - 1.0) - 0.5) / 0.5, 0.2); 
	
	float value = max(noise * mask, 0.0);
	value = smoothstep(0.5, 0.8, value);
	
	return value;
}

void fragment() {
	
	float wave = sin(UV.y * 40.0 - TIME * 1.0);
	
	float value = sample_value(UV + vec2(wave * 0.1 * waviness, 0.0));
	
	vec3 color = mix(secondary_color, primary_color, value);
	
	if(!FRONT_FACING){
		color = tertiary_color;
	}
	float hue_wave = sin(UV.x * TAU * 4.0 + TIME) + sin(UV.y * TAU * 5.0 + TIME);
	color = shift_hue(color, hue_wave * hue_variation);
	
	ALBEDO = color * emission;
	
	ALPHA = value;
	
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