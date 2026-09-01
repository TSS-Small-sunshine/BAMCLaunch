import { Badge, Box, Center, Text, VStack } from '@chakra-ui/react';

const KINDS = {
  download: {
    title: '资源下载',
    icon: 'M11 4h2v8.2l3.1-3.1 1.4 1.4L12 15.9 6.5 10.5l1.4-1.4L11 12.2V4ZM4 19h16v2H4v-2Z',
    blurb: '整合包 / Mod / 光影 / 资源包下载,参考 SJMCL 接入 CurseForge 与 Modrinth。',
  },
  accounts: {
    title: '账户管理',
    icon: 'M12 12a4 4 0 1 0-4-4 4 4 0 0 0 4 4Zm0 2c-4.42 0-8 2.24-8 5v1h16v-1c0-2.76-3.58-5-8-5Z',
    blurb: '微软账户登录与离线模式(参考 HMCL / SJMCL 的多账户体系)。',
  },
  settings: {
    title: '设置',
    icon: 'M4 6h9v2H4Zm11 0h5v2h-5ZM4 11h5v2H4Zm7 0h9v2h-9ZM4 16h9v2H4Zm11 0h5v2h-5Z',
    blurb: 'Java 路径、内存分配、游戏目录等个性化配置(后续里程碑开放)。',
  },
} as const;

export type PlaceholderKind = keyof typeof KINDS;

/** 通用占位页:告诉用户该功能未来长什么样 */
export default function PlaceholderPage({ kind }: { kind: PlaceholderKind }) {
  const { title, icon, blurb } = KINDS[kind];
  return (
    <Center h="100%" minH="60vh">
      <VStack spacing={5} maxW="420px" textAlign="center">
        <Badge colorScheme="grass" px={3} py={1}>
          规划中 · 后续里程碑
        </Badge>
        <Box
          w={16}
          h={16}
          borderRadius="2xl"
          bg="brand.50"
          color="brand.500"
          display="flex"
          alignItems="center"
          justifyContent="center"
          boxShadow="card"
        >
          <svg viewBox="0 0 24 24" width="30" height="30" fill="currentColor" aria-hidden>
            <path d={icon} />
          </svg>
        </Box>
        <Box>
          <Text fontSize="2xl" fontWeight="800" color="gray.800">
            {title}
          </Text>
          <Text fontSize="sm" color="gray.500" mt={2} lineHeight="1.7">
            {blurb}
          </Text>
        </Box>
      </VStack>
    </Center>
  );
}
