struct Vertex {
    float4 position;
    float4 color;
};

struct Scene {
    float4 transformRows[4];
};

typedef vk::BufferPointer<Vertex, 16> VertexBuffer;
typedef vk::BufferPointer<uint, 4> IndexBuffer;
typedef vk::BufferPointer<Scene, 16> SceneBuffer;

struct PushConstants {
    VertexBuffer vertices;
    IndexBuffer indices;
    SceneBuffer scene;
};

[[vk::push_constant]] PushConstants pushConstants;

struct VertexOutput {
    float3 color;
    float4 sv_position : SV_Position;
};

[shader("vertex")]
VertexOutput vertMain(uint vertexId : SV_VertexID) {
    uint index = pushConstants.indices.getPtr()[vertexId];
    Vertex vertex = pushConstants.vertices.getPtr()[index];
    Scene scene = pushConstants.scene.Get();

    VertexOutput output;

    output.sv_position = float4(
        dot(scene.transformRows[0], vertex.position),
        dot(scene.transformRows[1], vertex.position),
        dot(scene.transformRows[2], vertex.position),
        dot(scene.transformRows[3], vertex.position)
    );

    output.color = vertex.color.rgb;

    return output;
}

[shader("fragment")]
float4 fragMain(VertexOutput inVert) : SV_Target
{
    float3 color = inVert.color;

    return float4(color, 1.0);
}
