import { Link, Icon, Text, HStack, Tooltip, Box } from '@chakra-ui/react';
import React from 'react';
import styles from '../styles/header.module.css'

interface Props {
    url: string;
    text: string;
  }
  
export const HeaderLink = ({ text, url }: Props) => {
    return (
      <Box as="li" listStyleType="none">
          <Link href={url} className={styles.link}>
            {text}
          </Link>
      </Box>
    )
}