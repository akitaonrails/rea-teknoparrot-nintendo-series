// CRT look for the Initial D launcher: scanlines, an aperture-grille mask,
// a gentle barrel curve and a vignette, drawn by gamescope over the game
// (`--reshade-effect`). Self-contained ReShade FX (gamescope ships no
// ReShade.fxh). Written for this launcher after the approach of Timothy
// Lottes' public-domain CRT shader (Gaussian scanlines, RGB mask);
// GPL-3.0-or-later with the rest of the repository.

// Lines of the emulated tube: the arcade games ran at 480–768 lines; 480
// keeps the scanlines visible at 4K.
#ifndef CRT_LINES
#define CRT_LINES 480.0
#endif

texture BackBufferTex : COLOR;
sampler BackBuffer { Texture = BackBufferTex; AddressU = BORDER; AddressV = BORDER; };

static const float CURVE = 0.035;      // barrel distortion
static const float SCAN_HARD = -8.0;   // scanline sharpness (more negative: thinner)
static const float MASK_DARK = 0.70;   // mask: brightness of the off phosphors
static const float MASK_LIGHT = 1.30;  // mask: brightness of the lit one
static const float VIGNETTE = 0.20;

void PostProcessVS(in uint id : SV_VertexID, out float4 pos : SV_Position, out float2 uv : TEXCOORD)
{
    uv.x = (id == 2) ? 2.0 : 0.0;
    uv.y = (id == 1) ? 2.0 : 0.0;
    pos = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
}

float2 warp(float2 uv)
{
    float2 c = uv * 2.0 - 1.0;
    c *= 1.0 + CURVE * float2(c.y * c.y, c.x * c.x);
    return c * 0.5 + 0.5;
}

float3 to_linear(float3 c) { return c * c; }
float3 to_srgb(float3 c) { return sqrt(max(c, 0.0)); }

float3 mask(float2 pos)
{
    // aperture grille: one lit column of three
    float x = frac(pos.x / 3.0);
    float3 m = MASK_DARK.xxx;
    if (x < 0.333) m.r = MASK_LIGHT;
    else if (x < 0.666) m.g = MASK_LIGHT;
    else m.b = MASK_LIGHT;
    return m;
}

float3 CrtPS(float4 vpos : SV_Position, float2 uv : TEXCOORD) : SV_Target
{
    float2 w = warp(uv);
    if (any(w < 0.0) || any(w > 1.0))
        return 0.0.xxx;
    // sample at the centre of the emulated line, weight by the distance to it
    float line = w.y * CRT_LINES;
    float centre = floor(line) + 0.5;
    float d = line - centre;
    float3 col = to_linear(tex2D(BackBuffer, float2(w.x, centre / CRT_LINES)).rgb);
    float3 next = to_linear(tex2D(BackBuffer, float2(w.x, (centre + sign(d)) / CRT_LINES)).rgb);
    float wa = exp2(SCAN_HARD * d * d);
    float wb = exp2(SCAN_HARD * (1.0 - abs(d)) * (1.0 - abs(d)));
    col = col * wa + next * wb;
    col *= mask(vpos.xy);
    float2 v = w * (1.0 - w);
    col *= pow(saturate(v.x * v.y * 16.0), VIGNETTE);
    return to_srgb(col * 1.25);
}

technique CRT
{
    pass
    {
        VertexShader = PostProcessVS;
        PixelShader = CrtPS;
    }
}
