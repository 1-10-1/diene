typedef vk::BufferPointer<uint, 4> UintBuffer;

struct PushConstants {
  UintBuffer output;
  uint count;
  uint _padding;
};

[[vk::push_constant]]
PushConstants pushConstants;

// Slang emits `main` as this module's SPIR-V OpEntryPoint name
// regardless of the source function name (unlike main.slang, which has
// two entrypoints and keeps their source names). The backend looks up
// entrypoints by this same string for both compilation and the
// VkPipelineShaderStageCreateInfo::pName it hands to Vulkan, so the
// function must be named `main` to match.
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id: SV_DispatchThreadID) {
  uint index = id.x;

  if (index >= pushConstants.count) {
    return;
  }

  pushConstants.output.getPtr()[index] = index * index;
}
