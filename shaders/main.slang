struct Vertex {
    float4 position;
    float4 color;
    float4 uv;
};

struct Scene {
    float4 transformRows[4];
};

struct Material {
    uint albedoTextureIndex;
    uint3 _padding;
};

typedef vk::BufferPointer<Vertex, 16> VertexBuffer;
typedef vk::BufferPointer<uint, 4> IndexBuffer;
typedef vk::BufferPointer<Scene, 16> SceneBuffer;
typedef vk::BufferPointer<Material, 16> MaterialBuffer;

struct DrawItem {
    VertexBuffer vertices;
    IndexBuffer indices;
    uint materialIndex;
    uint3 _padding;
};

typedef vk::BufferPointer<DrawItem, 16> DrawBuffer;

struct PushConstants {
    DrawBuffer draws;
    MaterialBuffer materials;
    SceneBuffer scene;
};

[[vk::push_constant]] PushConstants pushConstants;

[[vk::binding(0, 0)]] Texture2D<float4> textures[];
[[vk::binding(1, 0)]] SamplerState textureSampler;

struct VertexOutput {
    float4 color;
    float2 uv;
    nointerpolation uint materialIndex;
    float4 sv_position : SV_Position;
};

[shader("vertex")]
VertexOutput vertMain(uint vertexId : SV_VertexID, uint drawId : SV_DrawIndex) {
    DrawItem draw = pushConstants.draws.getPtr()[drawId];
    uint index = draw.indices.getPtr()[vertexId];
    Vertex vertex = draw.vertices.getPtr()[index];
    Scene scene = pushConstants.scene.Get();

    VertexOutput output;

    output.sv_position = float4(
        dot(scene.transformRows[0], vertex.position),
        dot(scene.transformRows[1], vertex.position),
        dot(scene.transformRows[2], vertex.position),
        dot(scene.transformRows[3], vertex.position)
    );

    output.color = vertex.color;
    output.uv = vertex.uv.xy;
    output.materialIndex = draw.materialIndex;

    return output;
}

[shader("fragment")]
float4 fragMain(VertexOutput inVert) : SV_Target
{
    Material material = pushConstants.materials.getPtr()[inVert.materialIndex];
    float4 albedo = textures[NonUniformResourceIndex(material.albedoTextureIndex)].Sample(
        textureSampler,
        inVert.uv
    );

    return albedo * inVert.color;
}
