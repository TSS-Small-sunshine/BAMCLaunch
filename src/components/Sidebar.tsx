import { Box, Flex, Text, VStack } from "@chakra-ui/react";
import { NavLink } from "react-router-dom";

const NAV_ITEMS = [
  { to: "/", label: "版本", icon: "M12 3 2.5 11H5v9h5v-6h4v6h5v-9h2.5L12 3Z" },
  { to: "/download", label: "资源下载", icon: "M11 4h2v8.2l3.1-3.1 1.4 1.4L12 15.9 6.5 10.5l1.4-1.4L11 12.2V4ZM4 19h16v2H4v-2Z" },
  { to: "/accounts", label: "账户", icon: "M12 12a4 4 0 1 0-4-4 4 4 0 0 0 4 4Zm0 2c-4.42 0-8 2.24-8 5v1h16v-1c0-2.76-3.58-5-8-5Z" },
  { to: "/settings", label: "设置", icon: "M4 6h9v2H4Zm11 0h5v2h-5ZM4 11h5v2H4Zm7 0h9v2h-9ZM4 16h9v2H4Zm11 0h5v2h-5Z" },
];

function NavIcon({ d }: { d: string }) {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" aria-hidden>
      <path d={d} />
    </svg>
  );
}

/** 左侧导航:BA 风格白色面板 + 选中态蓝色胶囊 */
export default function Sidebar() {
  return (
    <Flex
      direction="column"
      w="220px"
      flexShrink={0}
      bg="white"
      borderRight="1px solid"
      borderColor="brand.100"
      p={4}
      gap={1}
    >
      {/* 品牌区 */}
      <Flex align="center" gap={3} px={2} py={3} mb={2}>
        <Box
          w={11}
          h={11}
          borderRadius="xl"
          bg="brand.500"
          color="white"
          display="flex"
          alignItems="center"
          justifyContent="center"
          fontWeight="800"
          fontSize="lg"
          boxShadow="glow"
        >
          B
        </Box>
        <Box>
          <Text fontWeight="800" fontSize="lg" color="gray.800" lineHeight={1.15}>
            BAMC Launch
          </Text>
          <Text fontSize="xs" fontWeight="700" color="grass.600" letterSpacing="1px">
            Minecraft 启动器
          </Text>
        </Box>
      </Flex>

      {/* 导航项 */}
      <VStack align="stretch" spacing={1.5}>
        {NAV_ITEMS.map((item) => (
          <NavLink key={item.to} to={item.to} style={{ textDecoration: "none" }}>
            {({ isActive }) => (
              <Flex
                px={3.5}
                py={2.5}
                borderRadius="xl"
                align="center"
                gap={3}
                fontSize="sm"
                fontWeight="700"
                bg={isActive ? "brand.500" : "transparent"}
                color={isActive ? "white" : "gray.600"}
                boxShadow={isActive ? "glow" : "none"}
                _hover={isActive ? undefined : { bg: "brand.50", color: "brand.600" }}
                transition="all 0.15s"
              >
                <NavIcon d={item.icon} />
                {item.label}
              </Flex>
            )}
          </NavLink>
        ))}
      </VStack>

      <Box flex={1} />

      {/* 底部:MC 像素点缀 + 版本号 */}
      <Box px={2} py={2}>
        <Flex gap="3px" mb={3}>
          {Array.from({ length: 14 }, (_, i) => (
            <Box key={i} w="6px" h="6px" borderRadius="1.5px" bg={i % 2 === 0 ? "grass.400" : "grass.600"} />
          ))}
        </Flex>
        <Text fontSize="xs" color="gray.400">
          v0.1.0 · 里程碑 1 骨架
        </Text>
      </Box>
    </Flex>
  );
}