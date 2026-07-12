struct Vertex {
    float4 position;
    float4 color;
};

typedef vk::BufferPointer<Vertex, 16> VertexBuffer;

struct PushConstants {
    VertexBuffer vertices;
};

[[vk::push_constant]] PushConstants pushConstants;

struct VertexOutput {
    float3 color;
    float4 sv_position : SV_Position;
};

[shader("vertex")]
VertexOutput vertMain(uint vertexId : SV_VertexID) {
    Vertex vertex = pushConstants.vertices.getPtr()[vertexId];

    VertexOutput output;
    output.sv_position = vertex.position;
    output.color = vertex.color.rgb;
    return output;
}

[shader("fragment")]
float4 fragMain(VertexOutput inVert) : SV_Target
{
    float3 color = inVert.color;
    return float4(color, 1.0);
}
