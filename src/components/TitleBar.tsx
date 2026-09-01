import { Box, HStack, IconButton } from '@chakra-ui/react';
import { CloseIcon, MinusIcon } from '@chakra-ui/icons';
import { getCurrentWindow } from '@tauri-apps/api/window';

const appWindow = getCurrentWindow();

/** 最大化/还原按钮的图标:一个空心方框 */
function MaximizeGlyph() {
  return (
    <Box w="10px" h="10px" border="1.5px solid" borderColor="currentColor" borderRadius="2px" />
  );
}

/**
 * 自绘标题栏:窗口未启用系统边框(decorations: false),
 * 整个横条标记 data-tauri-drag-region 即可被系统当作标题栏拖动
 */
export default function TitleBar() {
  return (
    <Box
      data-tauri-drag-region
      h="44px"
      flexShrink={0}
      display="flex"
      alignItems="center"
      justifyContent="space-between"
      pl={4}
      pr={2}
      bg="rgba(255, 255, 255, 0.8)"
      backdropFilter="blur(12px)"
      borderBottom="1px solid"
      borderColor="brand.100"
    >
      <Box data-tauri-drag-region display="flex" alignItems="center" gap={2}>
        <Box w="7px" h="7px" borderRadius="2px" bg="grass.500" boxShadow="0 0 0 2px #7CBD4B22" />
        <Box fontSize="xs" fontWeight="700" color="gray.500" letterSpacing="1.5px">
          BAMC LAUNCH
        </Box>
      </Box>

      <HStack spacing={0.5}>
        <IconButton
          aria-label="最小化"
          icon={<MinusIcon />}
          size="sm"
          variant="ghost"
          color="gray.500"
          onClick={() => appWindow.minimize()}
        />
        <IconButton
          aria-label="最大化"
          icon={<MaximizeGlyph />}
          size="sm"
          variant="ghost"
          color="gray.500"
          onClick={() => appWindow.toggleMaximize()}
        />
        <IconButton
          aria-label="关闭"
          icon={<CloseIcon />}
          size="sm"
          variant="ghost"
          color="gray.500"
          _hover={{ bg: 'red.500', color: 'white' }}
          onClick={() => appWindow.close()}
        />
      </HStack>
    </Box>
  );
}
